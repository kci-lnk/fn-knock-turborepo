import type { ExternalAuthProviderType } from "@/types";

export type OIDCProviderForm = {
  clientId: string;
  clientSecret: string;
  enabled?: boolean;
  id?: string;
  issuer: string;
  name: string;
  scopes: string;
  tenant: string;
  type: ExternalAuthProviderType;
};

export const normalizeOidcScopes = (value: string) =>
  value
    .split(/[,\s]+/u)
    .map((item) => item.trim())
    .filter(Boolean);

export const oidcConnectionValueText = (value: unknown) => {
  if (Array.isArray(value)) return value.join(" ");
  return typeof value === "string" ? value : "";
};

export const hasOidcConnectionValue = (value: unknown) => {
  if (Array.isArray(value)) return value.length > 0;
  return typeof value === "string" ? value.trim().length > 0 : Boolean(value);
};
