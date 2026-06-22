import { normalizeTotpAccessScopes } from "./totp-access-scopes";
import type { TOTPCredential } from "./config/types";

export const TOTP_CREDENTIAL_TRANSFER_KIND =
  "fn-knock.totp-credentials" as const;
export const TOTP_CREDENTIAL_TRANSFER_VERSION = 1 as const;
export const MAX_TOTP_CREDENTIAL_IMPORT_COUNT = 200;

export type TOTPCredentialTransferPayload = {
  kind: typeof TOTP_CREDENTIAL_TRANSFER_KIND;
  version: typeof TOTP_CREDENTIAL_TRANSFER_VERSION;
  exported_at: string;
  app_version?: string;
  credentials: TOTPCredential[];
};

export type TOTPCredentialImportSummary = {
  imported: number;
  skipped_existing_id: number;
  skipped_existing_secret: number;
  skipped_file_duplicate: number;
  invalid: number;
  total: number;
};

export class TOTPCredentialTransferError extends Error {
  constructor(
    public code:
      | "payloadObject"
      | "unsupportedKind"
      | "unsupportedVersion"
      | "credentialsArray"
      | "countExceeded",
    public params: Record<string, string | number | boolean> = {},
    public status = 400,
  ) {
    super(code);
    this.name = "TOTPCredentialTransferError";
  }
}

type BuildImportPlanResult = {
  credentials: TOTPCredential[];
  summary: TOTPCredentialImportSummary;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const nowIso = (now: Date) => now.toISOString();

const normalizeCreatedAt = (value: unknown, now: Date): string => {
  const createdAt = typeof value === "string" ? value.trim() : "";
  return Number.isFinite(Date.parse(createdAt)) ? createdAt : nowIso(now);
};

const createEmptySummary = (total: number): TOTPCredentialImportSummary => ({
  imported: 0,
  skipped_existing_id: 0,
  skipped_existing_secret: 0,
  skipped_file_duplicate: 0,
  invalid: 0,
  total,
});

export const buildTOTPCredentialTransferPayload = ({
  credentials,
  exportedAt = new Date(),
  appVersion,
}: {
  credentials: TOTPCredential[];
  exportedAt?: Date;
  appVersion?: string;
}): TOTPCredentialTransferPayload => ({
  kind: TOTP_CREDENTIAL_TRANSFER_KIND,
  version: TOTP_CREDENTIAL_TRANSFER_VERSION,
  exported_at: exportedAt.toISOString(),
  ...(appVersion ? { app_version: appVersion } : {}),
  credentials: credentials.map((credential) => ({
    id: String(credential.id ?? "").trim(),
    secret: String(credential.secret ?? "").trim(),
    comment: String(credential.comment ?? "").trim(),
    createdAt: normalizeCreatedAt(credential.createdAt, exportedAt),
    access_scopes: normalizeTotpAccessScopes(credential.access_scopes),
  })),
});

export const parseTOTPCredentialTransferPayload = (
  payload: unknown,
): TOTPCredentialTransferPayload => {
  if (!isRecord(payload)) {
    throw new TOTPCredentialTransferError("payloadObject");
  }
  if (payload.kind !== TOTP_CREDENTIAL_TRANSFER_KIND) {
    throw new TOTPCredentialTransferError("unsupportedKind");
  }
  if (payload.version !== TOTP_CREDENTIAL_TRANSFER_VERSION) {
    throw new TOTPCredentialTransferError("unsupportedVersion");
  }
  if (!Array.isArray(payload.credentials)) {
    throw new TOTPCredentialTransferError("credentialsArray");
  }
  if (payload.credentials.length > MAX_TOTP_CREDENTIAL_IMPORT_COUNT) {
    throw new TOTPCredentialTransferError("countExceeded", {
      max: MAX_TOTP_CREDENTIAL_IMPORT_COUNT,
    });
  }

  return payload as TOTPCredentialTransferPayload;
};

export const buildTOTPCredentialImportPlan = ({
  existing,
  payload,
  now = new Date(),
}: {
  existing: TOTPCredential[];
  payload: unknown;
  now?: Date;
}): BuildImportPlanResult => {
  const parsed = parseTOTPCredentialTransferPayload(payload);
  const summary = createEmptySummary(parsed.credentials.length);
  const existingIds = new Set(existing.map((credential) => credential.id));
  const knownSecrets = new Set(existing.map((credential) => credential.secret));
  const fileIds = new Set<string>();
  const credentials: TOTPCredential[] = [];

  for (const rawItem of parsed.credentials) {
    if (!isRecord(rawItem)) {
      summary.invalid += 1;
      continue;
    }

    const id = String(rawItem.id ?? "").trim();
    const secret = String(rawItem.secret ?? "").trim();
    if (!id || !secret) {
      summary.invalid += 1;
      continue;
    }

    if (fileIds.has(id)) {
      summary.skipped_file_duplicate += 1;
      continue;
    }
    fileIds.add(id);

    if (existingIds.has(id)) {
      summary.skipped_existing_id += 1;
      continue;
    }

    if (knownSecrets.has(secret)) {
      summary.skipped_existing_secret += 1;
      continue;
    }

    knownSecrets.add(secret);
    summary.imported += 1;
    credentials.push({
      id,
      secret,
      comment: String(rawItem.comment ?? "").trim(),
      createdAt: normalizeCreatedAt(rawItem.createdAt, now),
      access_scopes: normalizeTotpAccessScopes(rawItem.access_scopes),
    });
  }

  return { credentials, summary };
};
