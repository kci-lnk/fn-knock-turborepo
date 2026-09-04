import type { SessionRecord } from "../../types";

type SessionCredentialSource = Pick<
  SessionRecord,
  "method" | "credentialName" | "linkedTotpName"
>;

type Translator = (key: string, params?: Record<string, string>) => string;

const METHOD_LABEL_KEYS: Record<string, string> = {
  TOTP: "admin.sessions.credentialMethods.totp",
  PASSKEY: "admin.sessions.credentialMethods.passkey",
  PASSWORD: "admin.sessions.credentialMethods.password",
  OIDC: "admin.sessions.credentialMethods.oidc",
  LDAP: "admin.sessions.credentialMethods.ldap",
};

const normalizedText = (value: string | null | undefined) =>
  String(value ?? "").trim();

export const getSessionCredentialDisplayName = (
  session: SessionCredentialSource,
) =>
  normalizedText(session.linkedTotpName) ||
  normalizedText(session.credentialName) ||
  "-";

export const getSessionCredentialMethodLabel = (
  method: string | null | undefined,
  translate: Translator,
) => {
  const normalizedMethod = normalizedText(method).toUpperCase();
  const key = METHOD_LABEL_KEYS[normalizedMethod];
  return key ? translate(key) : normalizedText(method);
};

const formatMethodCredential = (
  method: string,
  name: string,
  translate: Translator,
) => {
  if (!method) return name;
  if (!name) return method;
  return translate("admin.sessions.credentialDisplay.methodWithCredential", {
    method,
    name,
  });
};

export const formatSessionCredentialLoginDetail = (
  session: SessionCredentialSource,
  translate: Translator,
) => {
  const normalizedMethod = normalizedText(session.method).toUpperCase();
  const method = getSessionCredentialMethodLabel(session.method, translate);
  const credentialName = normalizedText(session.credentialName);
  const linkedTotpName = normalizedText(session.linkedTotpName);
  const totpMethod = getSessionCredentialMethodLabel("TOTP", translate);

  if (normalizedMethod === "TOTP") {
    return formatMethodCredential(
      method || totpMethod,
      linkedTotpName || credentialName,
      translate,
    );
  }

  const loginDetail = formatMethodCredential(method, credentialName, translate);
  if (!linkedTotpName) {
    return loginDetail || getSessionCredentialDisplayName(session);
  }

  const parentDetail = formatMethodCredential(
    totpMethod,
    linkedTotpName,
    translate,
  );
  if (!loginDetail) return parentDetail;

  return translate("admin.sessions.credentialDisplay.relation", {
    parent: parentDetail,
    child: loginDetail,
  });
};
