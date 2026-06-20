import { createHash, randomBytes } from "node:crypto";
import { safeEqualString } from "../../security";
import { OIDC_CALLBACK_STATE_EXPIRED_MESSAGE } from "./messages";
import { normalizeString } from "./strings";

export const createId = (prefix: string) =>
  `${prefix}_${randomBytes(10).toString("hex")}`;

export const createPublicToken = () => randomBytes(32).toString("base64url");

export const hashOIDCToken = (value: string) =>
  createHash("sha256").update(value).digest("hex");

const base64Url = (value: Buffer) =>
  value
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");

export const createPkceVerifier = () => base64Url(randomBytes(32));

export const createPkceChallenge = (verifier: string) =>
  base64Url(createHash("sha256").update(verifier).digest());

export const isOIDCFlowTokenValid = (
  state: string | null | undefined,
  flowToken: string | null | undefined,
) => {
  const normalizedState = normalizeString(state);
  const normalizedFlowToken = normalizeString(flowToken);
  if (!normalizedState || !normalizedFlowToken) return false;
  return safeEqualString(hashOIDCToken(normalizedState), normalizedFlowToken);
};

export const assertOIDCFlowTokenValid = (
  state: string,
  flowToken: string | null | undefined,
) => {
  if (!isOIDCFlowTokenValid(state, flowToken)) {
    throw new Error(OIDC_CALLBACK_STATE_EXPIRED_MESSAGE);
  }
};

export const buildSubjectKey = (
  providerId: string,
  issuer: string,
  subject: string,
) =>
  createHash("sha256")
    .update(`${providerId}\u0000${issuer}\u0000${subject}`)
    .digest("hex");
