export const DOCKER_ADMIN_PANEL_ACCESS_SCOPE = "docker_admin_panel" as const;

export type TOTPAccessScope = typeof DOCKER_ADMIN_PANEL_ACCESS_SCOPE;

const VALID_TOTP_ACCESS_SCOPES = new Set<string>([
  DOCKER_ADMIN_PANEL_ACCESS_SCOPE,
]);

export type TotpAccessScopeCarrier = {
  id?: unknown;
  access_scopes?: unknown;
};

export type ReauthSessionScopeCarrier = {
  totpId?: unknown;
  expiresAt?: unknown;
};

export const normalizeTotpAccessScopes = (
  value: unknown,
): TOTPAccessScope[] => {
  if (!Array.isArray(value)) return [];

  const scopes = new Set<TOTPAccessScope>();
  for (const item of value) {
    const scope = String(item ?? "").trim();
    if (VALID_TOTP_ACCESS_SCOPES.has(scope)) {
      scopes.add(scope as TOTPAccessScope);
    }
  }

  return [...scopes];
};

export const hasTotpAccessScope = (
  credential: TotpAccessScopeCarrier | null | undefined,
  scope: TOTPAccessScope,
): boolean =>
  normalizeTotpAccessScopes(credential?.access_scopes).includes(scope);

export const isDockerAdminPanelReauthSessionAllowed = ({
  session,
  totpCredentials,
  now = Date.now(),
}: {
  session: ReauthSessionScopeCarrier | null | undefined;
  totpCredentials: TotpAccessScopeCarrier[];
  now?: number;
}): boolean => {
  const totpId = String(session?.totpId ?? "").trim();
  if (!totpId) return false;

  const expiresAt = Date.parse(String(session?.expiresAt ?? ""));
  if (!Number.isFinite(expiresAt) || expiresAt <= now) return false;

  const credential = totpCredentials.find((item) => item.id === totpId);
  return hasTotpAccessScope(credential, DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
};
