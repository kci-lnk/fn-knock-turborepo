import type { components as ApiContractComponents } from "@fn-knock/api-contract";

export type TOTPAccessScope =
  ApiContractComponents["schemas"]["AccessScopesUpdateData"]["access_scopes"][number];
export type TOTPSubdomainAccessMode =
  ApiContractComponents["schemas"]["TotpSubdomainAccessData"]["mode"];
export type TOTPStreamAccess =
  ApiContractComponents["schemas"]["TotpStreamAccessData"];
export type TOTPSubdomainAccess =
  ApiContractComponents["schemas"]["TotpSubdomainAccessData"];
export type TOTPCredential =
  ApiContractComponents["schemas"]["TotpCredentialData"];
export type TOTPCredentialImportSummary =
  ApiContractComponents["schemas"]["CredentialImportSummaryData"];

export type AuthLoginMode =
  ApiContractComponents["schemas"]["AuthLoginModeBody"]["mode"];

export type AuthLoginModeStatus = Omit<
  ApiContractComponents["schemas"]["AuthModeStatusData"],
  "mode"
> & {
  mode: AuthLoginMode;
};

export type AuthLoginModePreview = Omit<
  ApiContractComponents["schemas"]["AuthModePreviewData"],
  | "currentMode"
  | "targetMode"
  | "passwordRequiredBeforeSwitch"
  | "missingSourceTotpCount"
> & {
  currentMode: AuthLoginMode;
  targetMode: AuthLoginMode;
  passwordRequiredBeforeSwitch?: boolean;
  missingSourceTotpCount?: number;
};

export type AuthAccount = ApiContractComponents["schemas"]["AuthAccountData"];

export type PasskeyCredential =
  ApiContractComponents["schemas"]["PasskeyCredentialData"];

export type ExternalAuthProviderType =
  ApiContractComponents["schemas"]["OidcProviderCatalogItemData"]["type"];

export type ExternalAuthProtocol =
  ApiContractComponents["schemas"]["OidcProviderCatalogItemData"]["protocol"];

export type OIDCProviderCatalogItem =
  ApiContractComponents["schemas"]["OidcProviderCatalogItemData"];

type GeneratedOIDCProviderView =
  ApiContractComponents["schemas"]["OidcProviderData"];
export type OIDCProviderView = Omit<
  GeneratedOIDCProviderView,
  "connection_config_masked"
> & {
  connection_config_masked: GeneratedOIDCProviderView["connection_config_masked"] &
    Record<string, unknown>;
};

export type OIDCBinding = ApiContractComponents["schemas"]["OidcBindingData"];

export type LdapProviderType =
  ApiContractComponents["schemas"]["LdapProviderCatalogItemData"]["type"];

export type LdapProviderCatalogItem =
  ApiContractComponents["schemas"]["LdapProviderCatalogItemData"];

type GeneratedLdapProviderView =
  ApiContractComponents["schemas"]["LdapProviderData"];
export type LdapProviderView = Omit<
  GeneratedLdapProviderView,
  "connection_config"
> & {
  connection_config: GeneratedLdapProviderView["connection_config"] &
    Record<string, unknown>;
};

export type LdapBinding = ApiContractComponents["schemas"]["LdapBindingData"];

export type LoginSession = {
  totpId: string;
  method: "TOTP" | "PASSWORD" | "PASSKEY" | "OIDC" | "LDAP";
  credentialId: string;
  credentialName: string;
  comment?: string;
  ip: string;
  userAgent: string;
  loginTime: string;
  expiresAt?: string;
  ipLocation?: string;
};

export type SessionMobilitySummary = {
  hasHistory: boolean;
  driftCount: number;
  lastDriftAt: string | null;
  lastDriftSource:
    | "proxy-session"
    | "fnos-token"
    | "session-refresh"
    | "browser-session"
    | null;
};

export type SessionMobilityEvent =
  | {
      version: 1;
      kind: "login";
      happenedAt: string;
      source: "login";
      toIp: string;
      toIpLocation?: string;
    }
  | {
      version: 1;
      kind: "drift";
      happenedAt: string;
      source:
        "proxy-session" | "fnos-token" | "session-refresh" | "browser-session";
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

export type SessionMobilityDetails = {
  summary: SessionMobilitySummary;
  events: SessionMobilityEvent[];
};

export type SessionAppAttachmentRecord = {
  subjectHash: string;
  currentIp: string;
  createdAt: string;
  lastSeenAt: string;
  expiresAt: string | null;
};

export type SessionFnosAttachmentRecord = SessionAppAttachmentRecord;
export type SessionTrimMediaAttachmentRecord = SessionAppAttachmentRecord;

export type SessionRecord = LoginSession & {
  id: string;
  mobility?: SessionMobilitySummary;
  fnosAttachments?: SessionFnosAttachmentRecord[];
  trimMediaAttachments?: SessionTrimMediaAttachmentRecord[];
};
