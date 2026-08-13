import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import type {
  AuthAccount,
  AuthLoginMode,
  AuthLoginModePreview,
  AuthLoginModeStatus,
  LdapBinding,
  LdapProviderCatalogItem,
  LdapProviderView,
  OIDCBinding,
  OIDCProviderCatalogItem,
  OIDCProviderView,
  PasskeyCredential,
  TOTPCredential,
  TOTPCredentialImportSummary,
  TOTPSubdomainAccess,
  TOTPAccessScope,
} from "../../types";
import { apiClient } from "./client";

type AuthAccountCreateRequest =
  ApiContractComponents["schemas"]["AuthAccountCreateBody"];
type AuthAccountPatchRequest =
  ApiContractComponents["schemas"]["AuthAccountPatchBody"];
type AuthAccountSetupRequest =
  ApiContractComponents["schemas"]["AuthAccountSetupBody"];
type OidcProviderCreateRequest =
  ApiContractComponents["schemas"]["OidcProviderCreateData"];
type OidcProviderUpdateRequest =
  ApiContractComponents["schemas"]["OidcProviderUpdateData"];
type LdapProviderCreateRequest =
  ApiContractComponents["schemas"]["LdapProviderCreateData"];
type LdapProviderUpdateRequest =
  ApiContractComponents["schemas"]["LdapProviderUpdateData"];
type LdapProviderTestRequest =
  ApiContractComponents["schemas"]["LdapProviderTestBodyData"];
type ExternalAuthConnectionTest =
  ApiContractComponents["schemas"]["ExternalAuthConnectionTestData"];
type ExternalAuthInvitationRequest =
  ApiContractComponents["schemas"]["ExternalAuthInvitationBodyData"];
type ExternalAuthInvitation =
  ApiContractComponents["schemas"]["ExternalAuthInvitationData"];
export const configAuthApi = {
  async getTOTPStatus(): Promise<{
    bound: boolean;
    credentials: TOTPCredential[];
  }> {
    const res = await apiClient.get("/totp/status");
    return res.data.data;
  },
  async getAuthLoginMode(): Promise<AuthLoginModeStatus> {
    const res = await apiClient.get("/auth/mode");
    return res.data.data;
  },
  async previewAuthLoginMode(
    mode: AuthLoginMode,
  ): Promise<AuthLoginModePreview> {
    const res = await apiClient.post("/auth/mode/preview", { mode });
    return res.data.data;
  },
  async switchAuthLoginMode(mode: AuthLoginMode): Promise<AuthLoginModeStatus> {
    const res = await apiClient.post("/auth/mode/switch", { mode });
    return res.data.data;
  },
  async getAuthAccounts(): Promise<AuthAccount[]> {
    const res = await apiClient.get("/auth/accounts");
    return res.data.data.accounts || [];
  },
  async createAuthAccount(
    payload: AuthAccountCreateRequest,
  ): Promise<AuthAccount> {
    const res = await apiClient.post("/auth/accounts", payload);
    return res.data.data;
  },
  async updateAuthAccount(
    id: string,
    payload: AuthAccountPatchRequest,
  ): Promise<AuthAccount> {
    const res = await apiClient.patch(
      `/auth/accounts/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteAuthAccount(id: string): Promise<void> {
    await apiClient.delete(`/auth/accounts/${encodeURIComponent(id)}`);
  },
  async setAuthAccountPassword(
    id: string,
    password: string,
  ): Promise<AuthAccount> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/password`,
      { password },
    );
    return res.data.data;
  },
  async setupAuthAccount(
    id: string,
    payload: AuthAccountSetupRequest,
  ): Promise<AuthAccount> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/setup`,
      payload,
    );
    return res.data.data;
  },
  async setupAuthAccountTOTP(
    id: string,
  ): Promise<{ secret: string; uri: string }> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/totp/setup`,
    );
    return res.data.data;
  },
  async bindAuthAccountTOTP(
    id: string,
    secret: string,
    token: string,
  ): Promise<AuthAccount> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/totp/bind`,
      { secret, token },
    );
    return res.data.data;
  },
  async updateAuthAccountAccessScopes(
    id: string,
    accessScopes: TOTPAccessScope[],
  ): Promise<AuthAccount> {
    const res = await apiClient.patch(
      `/auth/accounts/${encodeURIComponent(id)}/access-scopes`,
      {
        access_scopes: accessScopes,
      },
    );
    return res.data.data;
  },
  async updateAuthAccountSubdomainAccess(
    id: string,
    subdomainAccess: TOTPSubdomainAccess,
  ): Promise<AuthAccount> {
    const res = await apiClient.patch(
      `/auth/accounts/${encodeURIComponent(id)}/subdomain-access`,
      {
        subdomain_access: subdomainAccess,
      },
    );
    return res.data.data;
  },
  async setupTOTP(): Promise<{ secret: string; uri: string }> {
    const res = await apiClient.post("/totp/setup");
    return res.data.data;
  },
  async bindTOTP(
    secret: string,
    token: string,
    comment?: string,
  ): Promise<{ success: boolean; message?: string }> {
    const res = await apiClient.post("/totp/bind", { secret, token, comment });
    return res.data;
  },
  async downloadTOTPCredentials(): Promise<Blob> {
    const res = await apiClient.get("/totp/credentials/export", {
      responseType: "blob",
    });
    return res.data;
  },
  async importTOTPCredentials(
    payload: unknown,
  ): Promise<TOTPCredentialImportSummary> {
    const res = await apiClient.post("/totp/credentials/import", { payload });
    return res.data.data;
  },
  async deleteTOTP(id: string): Promise<void> {
    await apiClient.delete(`/totp/${encodeURIComponent(id)}`);
  },
  async updateTOTPComment(id: string, comment: string): Promise<void> {
    await apiClient.patch(`/totp/${encodeURIComponent(id)}/comment`, {
      comment,
    });
  },
  async updateTOTPAccessScopes(
    id: string,
    accessScopes: TOTPAccessScope[],
  ): Promise<TOTPCredential> {
    const res = await apiClient.patch(
      `/totp/${encodeURIComponent(id)}/access-scopes`,
      {
        access_scopes: accessScopes,
      },
    );
    return res.data.data;
  },
  async updateTOTPSubdomainAccess(
    id: string,
    subdomainAccess: TOTPSubdomainAccess,
  ): Promise<TOTPCredential> {
    const res = await apiClient.patch(
      `/totp/${encodeURIComponent(id)}/subdomain-access`,
      {
        subdomain_access: subdomainAccess,
      },
    );
    return res.data.data;
  },
  async getPasskeys(totpId: string): Promise<PasskeyCredential[]> {
    const res = await apiClient.get(
      `/totp/${encodeURIComponent(totpId)}/passkeys`,
    );
    return res.data.data;
  },
  async deletePasskey(id: string): Promise<void> {
    await apiClient.delete(`/passkeys/${encodeURIComponent(id)}`);
  },
  async getOIDCProviderCatalog(): Promise<OIDCProviderCatalogItem[]> {
    const res = await apiClient.get("/auth/oidc/catalog");
    return res.data.data.providers;
  },
  async getOIDCProviders(): Promise<OIDCProviderView[]> {
    const res = await apiClient.get("/auth/oidc/providers");
    return res.data.data.providers;
  },
  async createOIDCProvider(
    payload: OidcProviderCreateRequest,
  ): Promise<OIDCProviderView> {
    const res = await apiClient.post("/auth/oidc/providers", payload);
    return res.data.data;
  },
  async updateOIDCProvider(
    id: string,
    payload: OidcProviderUpdateRequest,
  ): Promise<OIDCProviderView> {
    const res = await apiClient.patch(
      `/auth/oidc/providers/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteOIDCProvider(id: string): Promise<void> {
    await apiClient.delete(`/auth/oidc/providers/${encodeURIComponent(id)}`);
  },
  async testOIDCProvider(id: string): Promise<ExternalAuthConnectionTest> {
    const res = await apiClient.post(
      `/auth/oidc/providers/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async getOIDCBindings(totpId: string): Promise<OIDCBinding[]> {
    const res = await apiClient.get(
      `/auth/oidc/totp/${encodeURIComponent(totpId)}/bindings`,
    );
    return res.data.data.bindings;
  },
  async deleteOIDCBinding(id: string): Promise<void> {
    await apiClient.delete(`/auth/oidc/bindings/${encodeURIComponent(id)}`);
  },
  async createOIDCInvite(
    payload: ExternalAuthInvitationRequest,
  ): Promise<ExternalAuthInvitation> {
    const res = await apiClient.post("/auth/oidc/invitations", payload);
    return res.data.data;
  },
  async getLdapProviderCatalog(): Promise<LdapProviderCatalogItem[]> {
    const res = await apiClient.get("/auth/ldap/catalog");
    return res.data.data.providers;
  },
  async getLdapProviders(): Promise<LdapProviderView[]> {
    const res = await apiClient.get("/auth/ldap/providers");
    return res.data.data.providers;
  },
  async createLdapProvider(
    payload: LdapProviderCreateRequest,
  ): Promise<LdapProviderView> {
    const res = await apiClient.post("/auth/ldap/providers", payload);
    return res.data.data;
  },
  async updateLdapProvider(
    id: string,
    payload: LdapProviderUpdateRequest,
  ): Promise<LdapProviderView> {
    const res = await apiClient.patch(
      `/auth/ldap/providers/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteLdapProvider(id: string): Promise<void> {
    await apiClient.delete(`/auth/ldap/providers/${encodeURIComponent(id)}`);
  },
  async testLdapProvider(
    id: string,
    credentials?: LdapProviderTestRequest,
  ): Promise<ExternalAuthConnectionTest> {
    const res = await apiClient.post(
      `/auth/ldap/providers/${encodeURIComponent(id)}/test`,
      credentials ?? {},
    );
    return res.data;
  },
  async getLdapBindings(totpId: string): Promise<LdapBinding[]> {
    const res = await apiClient.get(
      `/auth/ldap/totp/${encodeURIComponent(totpId)}/bindings`,
    );
    return res.data.data.bindings;
  },
  async deleteLdapBinding(id: string): Promise<void> {
    await apiClient.delete(`/auth/ldap/bindings/${encodeURIComponent(id)}`);
  },
  async createLdapInvite(
    payload: ExternalAuthInvitationRequest,
  ): Promise<ExternalAuthInvitation> {
    const res = await apiClient.post("/auth/ldap/invitations", payload);
    return res.data.data;
  },
};
