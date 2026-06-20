import { configManager } from "../../redis";
import { resolveSafeRedirectUri } from "../../subdomain-mode";
import {
  getOIDCProviderDefinition,
  isExternalAuthProviderType,
  OIDC_PROVIDER_CATALOG,
} from "./catalog";
import {
  exchangeFormToken,
  fetchGithubProfile,
  getDiscovery,
  parseAccessToken,
  verifyStandardOidcProfile,
} from "./client";
import { OIDC_CALLBACK_STATE_EXPIRED_MESSAGE, oidcT } from "./messages";
import {
  applyExtraAuthParams,
  assertProviderReady,
  getMissingProviderRequiredFields,
  maskProvider,
  normalizeProviderConnectionConfig,
} from "./provider-config";
import { oidcRedisStore } from "./redis-store";
import { normalizeLoginErrorMessage, normalizeString } from "./strings";
import {
  assertOIDCFlowTokenValid,
  buildSubjectKey,
  createId,
  createPkceChallenge,
  createPkceVerifier,
  createPublicToken,
  hashOIDCToken,
  isOIDCFlowTokenValid,
} from "./tokens";
import type {
  ExternalAuthProfile,
  OIDCAuthState,
  OIDCAuthStateMode,
  OIDCBinding,
  OIDCBindInvite,
  OIDCProvider,
  OIDCProviderUpdateInput,
  OIDCProviderUpsertInput,
  OIDCProviderView,
} from "./types";
import { buildCallbackUrl, buildInviteUrl } from "./urls";

export {
  OIDC_CALLBACK_STATE_EXPIRED_MESSAGE,
  hashOIDCToken,
  isOIDCFlowTokenValid,
};

const STATE_TTL_SECONDS = 10 * 60;
export const OIDC_STATE_TTL_SECONDS = STATE_TTL_SECONDS;
const DEFAULT_INVITE_TTL_SECONDS = 30 * 60;
const MAX_INVITE_TTL_SECONDS = 7 * 24 * 60 * 60;
const LOGIN_ERROR_TTL_SECONDS = 5 * 60;

const nowIso = () => new Date().toISOString();

export class OIDCAuthService {
  listProviderCatalog() {
    return OIDC_PROVIDER_CATALOG;
  }

  async listProviders(request?: Request): Promise<OIDCProviderView[]> {
    const [providers, config] = await Promise.all([
      oidcRedisStore.listProviders(),
      request ? configManager.getConfig() : Promise.resolve(null),
    ]);
    return providers.map((provider) =>
      maskProvider(
        provider,
        request && config ? buildCallbackUrl(provider.id, request, config) : "",
      ),
    );
  }

  async listPublicProviders() {
    const providers = await oidcRedisStore.listProviders();
    return providers
      .filter((provider) => provider.enabled)
      .filter(
        (provider) => getMissingProviderRequiredFields(provider).length === 0,
      )
      .map((provider) => ({
        id: provider.id,
        type: provider.type,
        name: provider.name,
        protocol: provider.protocol,
      }));
  }

  async getProvider(id: string) {
    return oidcRedisStore.getProvider(id);
  }

  async createProvider(input: OIDCProviderUpsertInput) {
    if (!isExternalAuthProviderType(input.type)) {
      throw new Error(oidcT("providerUnsupported"));
    }
    const definition = getOIDCProviderDefinition(input.type);
    if (!definition) throw new Error(oidcT("providerUnsupported"));
    const now = nowIso();
    const enabled = input.enabled !== false;
    const provider: OIDCProvider = {
      id: createId("oidc_provider"),
      type: input.type,
      protocol: definition.protocol,
      name: normalizeString(input.name) || definition.default_name,
      enabled,
      connection_config: normalizeProviderConnectionConfig(
        input.type,
        input.connection_config,
        { allowIncomplete: !enabled },
      ),
      created_at: now,
      updated_at: now,
      last_test_status: "idle",
    };
    await oidcRedisStore.saveProvider(provider);
    return maskProvider(provider);
  }

  async updateProvider(id: string, input: OIDCProviderUpdateInput) {
    const provider = await oidcRedisStore.getProvider(id);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    const connectionPatch = input.connection_config || {};
    const nextEnabled =
      typeof input.enabled === "boolean" ? input.enabled : provider.enabled;
    const nextConnection = normalizeProviderConnectionConfig(
      provider.type,
      {
        ...provider.connection_config,
        ...connectionPatch,
      },
      { allowIncomplete: !nextEnabled },
    );
    const nextProvider: OIDCProvider = {
      ...provider,
      name:
        input.name !== undefined
          ? normalizeString(input.name) || provider.name
          : provider.name,
      enabled: nextEnabled,
      connection_config: nextConnection,
      updated_at: nowIso(),
    };
    await oidcRedisStore.saveProvider(nextProvider);
    return maskProvider(nextProvider);
  }

  async deleteProvider(id: string) {
    const provider = await oidcRedisStore.getProvider(id);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    await oidcRedisStore.deleteProvider(id);
  }

  async testProvider(id: string) {
    const provider = await oidcRedisStore.getProvider(id);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    let success = false;
    let message = oidcT("connectionTestSuccess");
    try {
      assertProviderReady(provider);
      if (provider.protocol === "oidc") {
        await getDiscovery(provider);
      } else {
        const cfg = provider.connection_config;
        if (!cfg.authorization_endpoint || !cfg.token_endpoint) {
          throw new Error(oidcT("oauthEndpointIncomplete"));
        }
      }
      success = true;
    } catch (error) {
      message = error instanceof Error ? error.message : oidcT("connectionTestFailed");
    }
    const updated: OIDCProvider = {
      ...provider,
      last_test_at: nowIso(),
      last_test_status: success ? "success" : "failed",
      last_error: success ? null : message,
      updated_at: nowIso(),
    };
    await oidcRedisStore.saveProvider(updated);
    return { success, message };
  }

  async createInvite(args: {
    request: Request;
    totpId: string;
    providerId: string;
    ttlSeconds?: number;
    note?: string;
  }) {
    const totp = (await configManager.getTOTPCredentials()).find(
      (item) => item.id === args.totpId,
    );
    if (!totp) throw new Error(oidcT("totpMissing"));
    const providerId = normalizeString(args.providerId);
    if (!providerId) {
      throw new Error(oidcT("selectProvider"));
    }
    const provider = await oidcRedisStore.getProvider(providerId);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    if (
      !provider.enabled ||
      getMissingProviderRequiredFields(provider).length
    ) {
      throw new Error(oidcT("providerUnavailable"));
    }
    const ttlSeconds = Math.min(
      Math.max(60, Math.floor(args.ttlSeconds || DEFAULT_INVITE_TTL_SECONDS)),
      MAX_INVITE_TTL_SECONDS,
    );
    const token = createPublicToken();
    const tokenHash = hashOIDCToken(token);
    const now = Date.now();
    const invite: OIDCBindInvite = {
      token_hash: tokenHash,
      totp_id: args.totpId,
      provider_id: providerId,
      created_at: new Date(now).toISOString(),
      expires_at: new Date(now + ttlSeconds * 1000).toISOString(),
      ...(normalizeString(args.note)
        ? { note: normalizeString(args.note) }
        : {}),
    };
    await oidcRedisStore.saveInvite(invite, ttlSeconds);
    const config = await configManager.getConfig();
    return {
      invite,
      token,
      invite_url: buildInviteUrl(token, args.request, config),
    };
  }

  async inspectInvite(token: string) {
    const tokenHash = hashOIDCToken(token);
    const invite = await oidcRedisStore.getInvite(tokenHash);
    if (!invite) return null;
    if (Date.parse(invite.expires_at) <= Date.now() || invite.used_at) {
      return null;
    }
    const [totps, providers] = await Promise.all([
      configManager.getTOTPCredentials(),
      oidcRedisStore.listProviders(),
    ]);
    const totp = totps.find((item) => item.id === invite.totp_id);
    if (!totp) return null;
    const allowedProviders = providers
      .filter((provider) => provider.enabled)
      .filter(
        (provider) => getMissingProviderRequiredFields(provider).length === 0,
      )
      .filter((provider) =>
        invite.provider_id ? provider.id === invite.provider_id : true,
      )
      .map((provider) => ({
        id: provider.id,
        type: provider.type,
        name: provider.name,
        protocol: provider.protocol,
      }));
    return {
      totp: { id: totp.id, comment: totp.comment },
      provider_id: invite.provider_id,
      expires_at: invite.expires_at,
      note: invite.note,
      providers: allowedProviders,
    };
  }

  async listBindingsByTotp(totpId: string) {
    const [bindings, providers, totps] = await Promise.all([
      oidcRedisStore.listBindingsByTotp(totpId),
      oidcRedisStore.listProviders(),
      configManager.getTOTPCredentials(),
    ]);
    return bindings.map((binding) => ({
      ...binding,
      provider_name:
        providers.find((provider) => provider.id === binding.provider_id)
          ?.name || binding.provider_type,
      totp_name: totps.find((totp) => totp.id === binding.totp_id)?.comment,
    }));
  }

  async deleteBinding(id: string) {
    const deleted = await oidcRedisStore.deleteBinding(id);
    if (!deleted) throw new Error(oidcT("bindingNotFound"));
  }

  async deleteBindingsByTotp(totpId: string) {
    return oidcRedisStore.deleteBindingsByTotp(totpId);
  }

  async buildAuthorizationUrl(args: {
    request: Request;
    providerId: string;
    mode: OIDCAuthStateMode;
    redirectUri?: string;
    inviteToken?: string;
    rememberMe?: boolean;
    clientIp?: string;
  }) {
    const [provider, config] = await Promise.all([
      oidcRedisStore.getProvider(args.providerId),
      configManager.getConfig(),
    ]);
    if (!provider || !provider.enabled) {
      throw new Error(oidcT("providerUnavailable"));
    }
    assertProviderReady(provider);
    let inviteTokenHash: string | undefined;
    if (args.mode === "bind") {
      const token = normalizeString(args.inviteToken);
      if (!token) throw new Error(oidcT("inviteInvalid"));
      const invite = await this.inspectInvite(token);
      if (!invite) throw new Error(oidcT("inviteExpired"));
      if (invite.provider_id && invite.provider_id !== provider.id) {
        throw new Error(oidcT("inviteProviderNotAllowed"));
      }
      inviteTokenHash = hashOIDCToken(token);
    }
    const callbackUrl = buildCallbackUrl(provider.id, args.request, config);
    const state = createPublicToken();
    const stateHash = hashOIDCToken(state);
    const nonce =
      provider.protocol === "oidc" ? createPublicToken() : undefined;
    const codeVerifier =
      provider.protocol === "oidc" ? createPkceVerifier() : undefined;
    const safeRedirectUri = resolveSafeRedirectUri({
      config,
      request: args.request,
      redirectUri: args.redirectUri,
    });
    const authState: OIDCAuthState = {
      state_hash: stateHash,
      mode: args.mode,
      provider_id: provider.id,
      ...(safeRedirectUri ? { redirect_uri: safeRedirectUri } : {}),
      ...(inviteTokenHash ? { invite_token_hash: inviteTokenHash } : {}),
      ...(codeVerifier ? { code_verifier: codeVerifier } : {}),
      ...(nonce ? { nonce } : {}),
      remember_me: args.rememberMe === true,
      ...(args.clientIp ? { client_ip: args.clientIp } : {}),
      created_at: nowIso(),
      expires_at: new Date(Date.now() + STATE_TTL_SECONDS * 1000).toISOString(),
    };
    await oidcRedisStore.saveState(authState, STATE_TTL_SECONDS);
    const authorizationUrl =
      provider.protocol === "oidc"
        ? await this.buildStandardOidcAuthorizationUrl(
            provider,
            callbackUrl,
            state,
            nonce || "",
            codeVerifier || "",
          )
        : this.buildOauthProfileAuthorizationUrl(provider, callbackUrl, state);
    return {
      authorization_url: authorizationUrl,
      flow_token: stateHash,
      max_age: STATE_TTL_SECONDS,
    };
  }

  async consumeCallbackState(args: {
    providerId: string;
    state: string;
    flowToken?: string | null;
  }): Promise<OIDCAuthState | null> {
    if (!isOIDCFlowTokenValid(args.state, args.flowToken)) {
      return null;
    }
    const stateHash = hashOIDCToken(args.state);
    const authState = await oidcRedisStore.consumeState(stateHash);
    if (!authState || authState.provider_id !== args.providerId) {
      return null;
    }
    return authState;
  }

  async createLoginErrorNotice(message: string) {
    const token = createPublicToken();
    const tokenHash = hashOIDCToken(token);
    const createdAt = new Date();
    const expiresAt = new Date(
      createdAt.getTime() + LOGIN_ERROR_TTL_SECONDS * 1000,
    );
    await oidcRedisStore.saveLoginErrorNotice(
      {
        token_hash: tokenHash,
        message: normalizeLoginErrorMessage(message),
        created_at: createdAt.toISOString(),
        expires_at: expiresAt.toISOString(),
      },
      LOGIN_ERROR_TTL_SECONDS,
    );
    return { token, maxAge: LOGIN_ERROR_TTL_SECONDS };
  }

  async consumeLoginErrorNotice(token?: string | null) {
    const normalizedToken = normalizeString(token);
    if (!normalizedToken) return null;

    const notice = await oidcRedisStore.consumeLoginErrorNotice(
      hashOIDCToken(normalizedToken),
    );
    return notice?.message || null;
  }

  private async buildStandardOidcAuthorizationUrl(
    provider: OIDCProvider,
    callbackUrl: string,
    state: string,
    nonce: string,
    codeVerifier: string,
  ) {
    const discovery = await getDiscovery(provider);
    const cfg = provider.connection_config;
    const params = new URLSearchParams({
      client_id: cfg.client_id,
      response_type: "code",
      redirect_uri: callbackUrl,
      scope: (cfg.scopes || ["openid", "profile", "email"]).join(" "),
      state,
      nonce,
      code_challenge: createPkceChallenge(codeVerifier),
      code_challenge_method: "S256",
    });
    applyExtraAuthParams(params, cfg.extra_auth_params);
    return `${discovery.authorization_endpoint}?${params.toString()}`;
  }

  private buildOauthProfileAuthorizationUrl(
    provider: OIDCProvider,
    callbackUrl: string,
    state: string,
  ) {
    const cfg = provider.connection_config;
    if (!cfg.authorization_endpoint) {
      throw new Error(oidcT("authorizationEndpointMissing"));
    }
    const params = new URLSearchParams({
      client_id: cfg.client_id,
      response_type: "code",
      redirect_uri: callbackUrl,
      scope: (cfg.scopes || []).join(" "),
      state,
    });
    applyExtraAuthParams(params, cfg.extra_auth_params);
    return `${cfg.authorization_endpoint}?${params.toString()}`;
  }

  async resolveCallback(args: {
    request: Request;
    providerId: string;
    code: string;
    state: string;
    flowToken?: string | null;
  }) {
    assertOIDCFlowTokenValid(args.state, args.flowToken);
    const stateHash = hashOIDCToken(args.state);
    const authState = await oidcRedisStore.consumeState(stateHash);
    if (!authState || authState.provider_id !== args.providerId) {
      throw new Error(OIDC_CALLBACK_STATE_EXPIRED_MESSAGE);
    }
    const provider = await oidcRedisStore.getProvider(args.providerId);
    if (!provider || !provider.enabled) {
      throw new Error(oidcT("providerUnavailable"));
    }
    const config = await configManager.getConfig();
    const callbackUrl = buildCallbackUrl(provider.id, args.request, config);
    const profile =
      provider.protocol === "oidc"
        ? await this.resolveStandardOidcCallback(
            provider,
            args.code,
            callbackUrl,
            authState,
          )
        : await this.resolveOauthProfileCallback(
            provider,
            args.code,
            callbackUrl,
          );
    const subjectKey = buildSubjectKey(
      provider.id,
      profile.issuer,
      profile.subject,
    );
    if (authState.mode === "bind") {
      if (!authState.invite_token_hash) {
        throw new Error(oidcT("bindStateInvalid"));
      }
      return this.bindProfileAndResolveLogin({
        provider,
        profile,
        subjectKey,
        state: authState,
      });
    }
    const binding = await oidcRedisStore.getBindingBySubject(subjectKey);
    if (!binding) {
      throw new Error(oidcT("accountNotBoundCannotLogin"));
    }
    await oidcRedisStore.saveBinding({
      ...binding,
      display_name: profile.display_name || binding.display_name,
      email: profile.email || binding.email,
      email_verified: profile.email_verified ?? binding.email_verified,
      avatar_url: profile.avatar_url || binding.avatar_url,
      last_used_at: nowIso(),
      updated_at: nowIso(),
    });
    return {
      state: authState,
      provider,
      binding,
      profile,
    };
  }

  private async resolveStandardOidcCallback(
    provider: OIDCProvider,
    code: string,
    callbackUrl: string,
    authState: OIDCAuthState,
  ) {
    const discovery = await getDiscovery(provider);
    const cfg = provider.connection_config;
    const body = new URLSearchParams({
      grant_type: "authorization_code",
      client_id: cfg.client_id,
      client_secret: cfg.client_secret,
      code,
      redirect_uri: callbackUrl,
    });
    if (authState.code_verifier) {
      body.set("code_verifier", authState.code_verifier);
    }
    const tokenPayload = await exchangeFormToken(
      discovery.token_endpoint,
      body,
    );
    return verifyStandardOidcProfile(
      provider,
      tokenPayload,
      discovery,
      authState.nonce,
    );
  }

  private async resolveOauthProfileCallback(
    provider: OIDCProvider,
    code: string,
    callbackUrl: string,
  ) {
    const cfg = provider.connection_config;
    if (!cfg.token_endpoint) throw new Error(oidcT("tokenEndpointMissing"));
    const body = new URLSearchParams({
      grant_type: "authorization_code",
      client_id: cfg.client_id,
      client_secret: cfg.client_secret,
      code,
      redirect_uri: callbackUrl,
    });
    const tokenPayload = await exchangeFormToken(
      cfg.token_endpoint,
      body,
      provider.type === "github" ? { Accept: "application/json" } : undefined,
    );
    const accessToken = parseAccessToken(tokenPayload);
    if (provider.type === "github") {
      return fetchGithubProfile(provider, accessToken);
    }
    throw new Error(oidcT("providerUnsupported"));
  }

  private async bindProfileAndResolveLogin(args: {
    provider: OIDCProvider;
    profile: ExternalAuthProfile;
    subjectKey: string;
    state: OIDCAuthState;
  }) {
    const invite = await oidcRedisStore.getInvite(
      args.state.invite_token_hash!,
    );
    if (!invite) throw new Error(oidcT("inviteExpired"));
    if (invite.provider_id && invite.provider_id !== args.provider.id) {
      throw new Error(oidcT("bindProviderMismatch"));
    }
    const totp = (await configManager.getTOTPCredentials()).find(
      (item) => item.id === invite.totp_id,
    );
    if (!totp) throw new Error(oidcT("inviteTotpMissing"));
    const existing = await oidcRedisStore.getBindingBySubject(args.subjectKey);
    if (existing && existing.totp_id !== invite.totp_id) {
      throw new Error(oidcT("accountAlreadyBoundOtherTotp"));
    }
    const consumed = await oidcRedisStore.consumeInvite(
      args.state.invite_token_hash!,
    );
    if (!consumed) throw new Error(oidcT("inviteUsed"));
    if (existing) {
      const updated: OIDCBinding = {
        ...existing,
        display_name: args.profile.display_name || existing.display_name,
        email: args.profile.email || existing.email,
        email_verified: args.profile.email_verified ?? existing.email_verified,
        avatar_url: args.profile.avatar_url || existing.avatar_url,
        last_used_at: nowIso(),
        updated_at: nowIso(),
      };
      await oidcRedisStore.saveBinding(updated);
      return {
        state: args.state,
        provider: args.provider,
        binding: updated,
        profile: args.profile,
      };
    }
    const now = nowIso();
    const binding: OIDCBinding = {
      id: createId("oidc_binding"),
      provider_id: args.provider.id,
      provider_type: args.provider.type,
      totp_id: invite.totp_id,
      issuer: args.profile.issuer,
      subject: args.profile.subject,
      subject_key: args.subjectKey,
      ...(args.profile.display_name
        ? { display_name: args.profile.display_name }
        : {}),
      ...(args.profile.email ? { email: args.profile.email } : {}),
      ...(typeof args.profile.email_verified === "boolean"
        ? { email_verified: args.profile.email_verified }
        : {}),
      ...(args.profile.avatar_url
        ? { avatar_url: args.profile.avatar_url }
        : {}),
      created_at: now,
      updated_at: now,
      last_used_at: now,
    };
    const saved = await oidcRedisStore.saveBindingIfSubjectAvailable(binding);
    if (!saved) {
      const raced = await oidcRedisStore.getBindingBySubject(args.subjectKey);
      if (raced && raced.totp_id === invite.totp_id) {
        const updated: OIDCBinding = {
          ...raced,
          display_name: args.profile.display_name || raced.display_name,
          email: args.profile.email || raced.email,
          email_verified: args.profile.email_verified ?? raced.email_verified,
          avatar_url: args.profile.avatar_url || raced.avatar_url,
          last_used_at: now,
          updated_at: now,
        };
        await oidcRedisStore.saveBinding(updated);
        return {
          state: args.state,
          provider: args.provider,
          binding: updated,
          profile: args.profile,
        };
      }
      throw new Error(oidcT("accountAlreadyBoundOtherTotp"));
    }
    return {
      state: args.state,
      provider: args.provider,
      binding,
      profile: args.profile,
    };
  }
}

export const oidcAuthService = new OIDCAuthService();
