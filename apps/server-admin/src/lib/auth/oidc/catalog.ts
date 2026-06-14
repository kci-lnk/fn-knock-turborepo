import type {
  ExternalAuthProviderType,
  OIDCProviderCatalogItem,
  OIDCProviderConnectionConfig,
} from "./types";
import { tDefault } from "../../i18n";

const oidcCatalogT = (key: string): string =>
  tDefault(`server.oidc.catalog.${key}`);

export const OIDC_PROVIDER_CATALOG: OIDCProviderCatalogItem[] = [
  {
    type: "google",
    protocol: "oidc",
    label: "Google",
    description: oidcCatalogT("googleDescription"),
    default_name: "Google",
    default_scopes: ["openid", "profile", "email"],
    required_fields: ["client_id", "client_secret"],
    optional_fields: ["issuer", "scopes", "extra_auth_params"],
    supports_pkce: true,
    supports_discovery: true,
  },
  {
    type: "microsoft",
    protocol: "oidc",
    label: "Microsoft",
    description: oidcCatalogT("microsoftDescription"),
    default_name: "Microsoft",
    default_scopes: ["openid", "profile", "email"],
    required_fields: ["client_id", "client_secret"],
    optional_fields: ["tenant", "issuer", "scopes", "extra_auth_params"],
    supports_pkce: true,
    supports_discovery: true,
  },
  {
    type: "github",
    protocol: "oauth2_profile",
    label: "GitHub",
    description: oidcCatalogT("githubDescription"),
    default_name: "GitHub",
    default_scopes: ["read:user", "user:email"],
    required_fields: ["client_id", "client_secret"],
    optional_fields: ["scopes", "extra_auth_params"],
    supports_pkce: false,
    supports_discovery: false,
  },
  {
    type: "custom_oidc",
    protocol: "oidc",
    label: oidcCatalogT("customLabel"),
    description: oidcCatalogT("customDescription"),
    default_name: oidcCatalogT("customLabel"),
    default_scopes: ["openid", "profile", "email"],
    required_fields: ["client_id", "client_secret", "issuer"],
    optional_fields: [
      "authorization_endpoint",
      "token_endpoint",
      "userinfo_endpoint",
      "jwks_uri",
      "scopes",
      "extra_auth_params",
    ],
    supports_pkce: true,
    supports_discovery: true,
  },
];

export const getOIDCProviderDefinition = (type: string) =>
  OIDC_PROVIDER_CATALOG.find((item) => item.type === type) || null;

export const isExternalAuthProviderType = (
  type: string,
): type is ExternalAuthProviderType =>
  OIDC_PROVIDER_CATALOG.some((item) => item.type === type);

export const getDefaultConnectionConfig = (
  type: ExternalAuthProviderType,
): Partial<OIDCProviderConnectionConfig> => {
  switch (type) {
    case "google":
      return {
        issuer: "https://accounts.google.com",
      };
    case "microsoft":
      return {
        tenant: "common",
        issuer: "https://login.microsoftonline.com/common/v2.0",
      };
    case "github":
      return {
        authorization_endpoint: "https://github.com/login/oauth/authorize",
        token_endpoint: "https://github.com/login/oauth/access_token",
        userinfo_endpoint: "https://api.github.com/user",
        emails_endpoint: "https://api.github.com/user/emails",
      };
    case "custom_oidc":
    default:
      return {};
  }
};
