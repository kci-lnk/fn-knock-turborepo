import { Elysia, t } from "elysia";
import { configManager } from "../../lib/redis";
import { applyNoStoreHeaders } from "../../lib/auth-access";
import { getClientIp } from "../../lib/auth-request";
import {
  handleLoginSuccess,
  resolveTotpCredentialName,
} from "../../lib/auth-utils";
import {
  normalizeAuthFailureTrackingIp,
  registerAuthFailure,
} from "../../lib/auth-failure";
import { loginBackoffService } from "../../lib/login-backoff";
import {
  OIDC_CALLBACK_STATE_EXPIRED_MESSAGE,
  isOIDCFlowTokenValid,
  oidcAuthService,
} from "../../lib/auth/oidc/service";
import {
  resolveCookieDomain,
  resolvePublicAuthBaseUrl,
} from "../../lib/subdomain-mode";
import {
  OIDC_FLOW_COOKIE_NAME,
  buildOidcFlowClearCookie,
  buildOidcFlowCookie,
  buildOidcLoginErrorCookie,
  readCookieValue,
} from "../../lib/session-cookie";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import { createRequestTranslator } from "../../lib/i18n";
import { normalizeLocaleConfig } from "../../../../../packages/i18n/src";

const resolveAuthViewPrefix = (request: Request) => {
  const pathname = new URL(request.url).pathname;
  if (pathname.startsWith("/__auth__/")) return "/__auth__";
  if (pathname.startsWith("/auth/")) return "/auth";
  return "";
};

const resolveConfiguredAuthViewPrefix = (
  request: Request,
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
) => {
  const requestPrefix = resolveAuthViewPrefix(request);
  if (requestPrefix) return requestPrefix;
  const publicBaseUrl = resolvePublicAuthBaseUrl(config);
  if (!publicBaseUrl) return "";
  try {
    const pathname = new URL(publicBaseUrl).pathname.replace(/\/+$/, "");
    if (pathname && pathname !== "/") return pathname;
  } catch {
    // ignore invalid configured public url
  }
  return "";
};

const buildLoginRedirect = (
  request: Request,
  params: Record<string, string | undefined>,
  config?: Awaited<ReturnType<typeof configManager.getConfig>>,
) => {
  const prefix = config
    ? resolveConfiguredAuthViewPrefix(request, config)
    : resolveAuthViewPrefix(request);
  const url = new URL(`${prefix}/login`, request.url);
  for (const [key, value] of Object.entries(params)) {
    if (value) url.searchParams.set(key, value);
  }
  return `${url.pathname}${url.search}`;
};

const buildRedirectResponse = (
  location: string,
  setHeaders?: Record<string, string | number | boolean | string[] | undefined>,
  extraSetCookies: string[] = [],
) => {
  const headers = new Headers({ Location: location });
  applyNoStoreHeaders(headers);
  const setCookie = setHeaders?.["set-cookie"] ?? setHeaders?.["Set-Cookie"];
  const setCookieValues = Array.isArray(setCookie)
    ? setCookie
    : setCookie
      ? [String(setCookie)]
      : [];
  for (const cookie of [...setCookieValues, ...extraSetCookies]) {
    headers.append("Set-Cookie", cookie);
  }
  return new Response("", { status: 302, headers });
};

const appendSetCookieHeader = (
  set: {
    headers: Record<string, string | number | boolean | string[] | undefined>;
  },
  cookie: string,
) => {
  const current = set.headers["set-cookie"] ?? set.headers["Set-Cookie"];
  delete set.headers["Set-Cookie"];
  if (!current) {
    set.headers["set-cookie"] = cookie;
    return;
  }

  set.headers["set-cookie"] = Array.isArray(current)
    ? [...current, cookie]
    : [String(current), cookie];
};

const resolveOidcCookiePath = (
  request: Request,
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
) => resolveConfiguredAuthViewPrefix(request, config) || "/";

const buildOidcFlowCookieForRequest = (
  token: string,
  maxAge: number,
  request: Request,
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
) =>
  buildOidcFlowCookie(token, maxAge, {
    domain: resolveCookieDomain(config, request),
    path: resolveOidcCookiePath(request, config),
  });

const buildOidcFlowClearCookieForRequest = (
  request: Request,
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
) =>
  buildOidcFlowClearCookie({
    domain: resolveCookieDomain(config, request),
    path: resolveOidcCookiePath(request, config),
  });

const resolveProviderErrorMessage = (
  error: string | undefined,
  t: ReturnType<typeof createRequestTranslator>["t"],
) => {
  switch (
    String(error || "")
      .trim()
      .toLowerCase()
  ) {
    case "access_denied":
      return t("server.oidc.providerErrors.accessDenied");
    case "temporarily_unavailable":
      return t("server.oidc.providerErrors.temporarilyUnavailable");
    case "server_error":
      return t("server.oidc.providerErrors.serverError");
    case "invalid_scope":
      return t("server.oidc.providerErrors.invalidScope");
    case "invalid_request":
    case "unauthorized_client":
    case "unsupported_response_type":
      return t("server.oidc.providerErrors.rejected");
    default:
      return t("server.oidc.providerErrors.incomplete");
  }
};

const getErrorMessage = (error: unknown, fallback: string) =>
  error instanceof Error ? error.message : fallback;

const isOidcOperationAbortedError = (error: unknown): boolean => {
  if (error instanceof Error) {
    const message = `${error.name} ${error.message}`.toLowerCase();
    if (message.includes("operation was aborted")) return true;
    if (error.name === "AbortError" && message.includes("aborted")) return true;

    const cause = (error as Error & { cause?: unknown }).cause;
    return cause ? isOidcOperationAbortedError(cause) : false;
  }

  return String(error ?? "")
    .toLowerCase()
    .includes("operation was aborted");
};

const buildLoginErrorRedirectResponse = async ({
  request,
  config,
  message,
  redirectUri,
  persistNotice = true,
  extraSetCookies = [],
}: {
  request: Request;
  config: Awaited<ReturnType<typeof configManager.getConfig>>;
  message: string;
  redirectUri?: string;
  persistNotice?: boolean;
  extraSetCookies?: string[];
}) => {
  const setCookies: string[] = [];
  if (persistNotice) {
    try {
      const notice = await oidcAuthService.createLoginErrorNotice(message);
      setCookies.push(
        buildOidcLoginErrorCookie(notice.token, notice.maxAge, {
          domain: resolveCookieDomain(config, request),
          path: resolveOidcCookiePath(request, config),
        }),
      );
    } catch (error) {
      console.error(
        "[auth][oidc] failed to persist login error notice:",
        error,
      );
    }
  }
  setCookies.push(...extraSetCookies);

  return buildRedirectResponse(
    buildLoginRedirect(
      request,
      redirectUri ? { redirect_uri: redirectUri } : {},
      config,
    ),
    undefined,
    setCookies,
  );
};

type OIDCInviteDetails = NonNullable<
  Awaited<ReturnType<typeof oidcAuthService.inspectInvite>>
>;

const escapeHtml = (value: unknown) =>
  String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");

const buildBindHtmlResponse = (
  status: number,
  title: string,
  body: string,
  locale = "zh-CN",
  actions = "",
) => {
  const headers = new Headers({
    "content-type": "text/html; charset=utf-8",
    "content-security-policy":
      "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "referrer-policy": "no-referrer",
  });
  applyNoStoreHeaders(headers);
  return new Response(
    `<!doctype html>
<html lang="${escapeHtml(locale)}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeHtml(title)}</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#f6f7f9;color:#111827;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{width:min(92vw,420px);box-sizing:border-box;border:1px solid #e5e7eb;border-radius:12px;background:#fff;padding:28px;box-shadow:0 18px 48px rgba(15,23,42,.08)}
      h1{margin:0 0 10px;font-size:22px;line-height:1.25}
      p{margin:0;color:#4b5563;line-height:1.7;font-size:14px}
      .actions{display:grid;gap:10px;margin-top:22px}
      a{display:flex;align-items:center;justify-content:center;height:40px;border-radius:8px;background:#111827;color:#fff;text-decoration:none;font-size:14px;font-weight:600}
      a.secondary{background:#f3f4f6;color:#111827}
    </style>
  </head>
  <body>
    <main>
      <h1>${escapeHtml(title)}</h1>
      <p>${escapeHtml(body)}</p>
      ${actions}
    </main>
  </body>
</html>`,
    { status, headers },
  );
};

const buildBindProviderSelectionResponse = (
  request: Request,
  token: string,
  invite: OIDCInviteDetails,
  t: ReturnType<typeof createRequestTranslator>["t"],
  locale: string,
) => {
  const actions = invite.providers
    .map((provider) => {
      const url = new URL(request.url);
      url.search = "";
      url.searchParams.set("token", token);
      url.searchParams.set("provider_id", provider.id);
      return `<a href="${escapeHtml(`${url.pathname}${url.search}`)}">${escapeHtml(
        t("server.oidc.bindWithProvider", { provider: provider.name }),
      )}</a>`;
    })
    .join("");

  return buildBindHtmlResponse(
    200,
    t("server.oidc.selectProviderTitle"),
    t("server.oidc.bindToTotp", { totp: invite.totp.comment || "TOTP" }),
    locale,
    `<div class="actions">${actions}</div>`,
  );
};

export const oidcRoutes = new Elysia({
  prefix: "/oidc",
  tags: ["Auth - OIDC"],
})
  .onBeforeHandle(({ set }) => {
    applyNoStoreHeaders(set.headers);
  })
  .get(
    "/providers",
    async () => ({
      success: true,
      data: {
        providers: await oidcAuthService.listPublicProviders(),
      },
    }),
    routeDoc("获取公开外部登录提供商"),
  )
  .get(
    "/invite",
    async ({ query, set, request }) => {
      const config = await configManager.getConfig();
      const locale = normalizeLocaleConfig(config.locale);
      const { t } = createRequestTranslator(request, config.locale);
      const token = query.token?.trim();
      if (!token) {
        set.status = 400;
        return {
          success: false,
          message: t("server.oidc.inviteInvalid"),
          data: { locale },
        };
      }
      const invite = await oidcAuthService.inspectInvite(token);
      if (!invite) {
        set.status = 404;
        return {
          success: false,
          message: t("server.oidc.inviteExpired"),
          data: { locale },
        };
      }
      return {
        success: true,
        data: {
          locale,
          ...invite,
        },
      };
    },
    withRouteDoc("检查外部账号绑定邀请", {
      query: t.Object({
        token: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/bind",
    async ({ query, request }) => {
      const config = await configManager.getConfig();
      const { locale, t } = createRequestTranslator(request, config.locale);
      const token = query.token?.trim();
      if (!token) {
        return buildBindHtmlResponse(
          400,
          t("server.oidc.inviteInvalid"),
          t("server.oidc.linkMissingToken"),
          locale,
        );
      }

      const invite = await oidcAuthService.inspectInvite(token);
      if (!invite) {
        return buildBindHtmlResponse(
          404,
          t("server.oidc.inviteExpired"),
          t("server.oidc.inviteMissingExpiredUsed"),
          locale,
        );
      }
      if (invite.providers.length === 0) {
        return buildBindHtmlResponse(
          404,
          t("server.oidc.noProvidersTitle"),
          t("server.oidc.noProvidersBody"),
          locale,
        );
      }

      const selectedProviderId =
        query.provider_id?.trim() ||
        invite.provider_id ||
        (invite.providers.length === 1 ? invite.providers[0]?.id : "");

      if (!selectedProviderId) {
        return buildBindProviderSelectionResponse(
          request,
          token,
          invite,
          t,
          locale,
        );
      }

      try {
        const clientIp = getClientIp(request);
        const result = await oidcAuthService.buildAuthorizationUrl({
          request,
          providerId: selectedProviderId,
          mode: "bind",
          inviteToken: token,
          rememberMe: false,
          clientIp,
        });
        return buildRedirectResponse(result.authorization_url, undefined, [
          buildOidcFlowCookieForRequest(
            result.flow_token,
            result.max_age,
            request,
            config,
          ),
        ]);
      } catch (error) {
        return buildBindHtmlResponse(
          400,
          t("server.oidc.bindFailedTitle"),
          error instanceof Error
            ? error.message
            : t("server.oidc.bindStartFailed"),
          locale,
        );
      }
    },
    withRouteDoc("发起外部账号绑定跳转", {
      query: t.Object({
        token: t.Optional(t.String()),
        provider_id: t.Optional(t.String()),
      }),
    }),
  )
  .post(
    "/start",
    async ({ body, request, set }) => {
      const config = await configManager.getConfig();
      const { t } = createRequestTranslator(request, config.locale);
      try {
        const clientIp = getClientIp(request);
        const result = await oidcAuthService.buildAuthorizationUrl({
          request,
          providerId: body.provider_id,
          mode: body.mode || "login",
          redirectUri: body.redirect_uri,
          inviteToken: body.invite_token,
          rememberMe: body.rememberMe,
          clientIp,
        });
        appendSetCookieHeader(
          set,
          buildOidcFlowCookieForRequest(
            result.flow_token,
            result.max_age,
            request,
            config,
          ),
        );
        return {
          success: true,
          data: { authorization_url: result.authorization_url },
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.startFailed"),
        };
      }
    },
    withRouteDoc("发起外部登录授权", {
      body: t.Object({
        provider_id: t.String(),
        mode: t.Optional(t.Union([t.Literal("login"), t.Literal("bind")])),
        invite_token: t.Optional(t.String()),
        redirect_uri: t.Optional(t.String()),
        rememberMe: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/callback/:providerId",
    async ({ params, query, request, set }) => {
      const clientIp = getClientIp(request);
      const userAgent = request.headers.get("user-agent") || "Unknown";
      const code = query.code?.trim();
      const state = query.state?.trim();
      const providerName = params.providerId;
      const config = await configManager.getConfig();
      const flowToken = readCookieValue(
        request.headers.get("cookie"),
        OIDC_FLOW_COOKIE_NAME,
      );
      const { t } = createRequestTranslator(request, config.locale);
      const resolveFlowClearCookies = () =>
        state && isOIDCFlowTokenValid(state, flowToken)
          ? [buildOidcFlowClearCookieForRequest(request, config)]
          : [];
      const consumeStateForErrorNotice = async () => {
        if (!state) return null;
        try {
          return await oidcAuthService.consumeCallbackState({
            providerId: params.providerId,
            state,
            flowToken,
          });
        } catch (error) {
          console.warn("[auth][oidc] failed to consume error callback state", {
            providerId: params.providerId,
            error,
          });
          return null;
        }
      };

      if (query.error) {
        const authState = await consumeStateForErrorNotice();
        return buildLoginErrorRedirectResponse({
          request,
          config,
          message: resolveProviderErrorMessage(query.error, t),
          redirectUri: authState?.redirect_uri,
          persistNotice: !!authState,
          extraSetCookies: resolveFlowClearCookies(),
        });
      }

      if (!code || !state) {
        const callbackUrl = new URL(request.url);
        console.warn("[auth][oidc] callback missing required params", {
          providerId: params.providerId,
          pathname: callbackUrl.pathname,
          queryKeys: [...callbackUrl.searchParams.keys()],
          hasCode: Boolean(code),
          hasState: Boolean(state),
          forwardedHost: request.headers.get("x-forwarded-host"),
          forwardedProto: request.headers.get("x-forwarded-proto"),
        });
        const authState = await consumeStateForErrorNotice();
        return buildLoginErrorRedirectResponse({
          request,
          config,
          message: t("server.oidc.callbackMissingParams"),
          redirectUri: authState?.redirect_uri,
          persistNotice: !!authState,
          extraSetCookies: resolveFlowClearCookies(),
        });
      }

      const gate = await loginBackoffService.ensureNotBlocked(
        normalizeAuthFailureTrackingIp(clientIp),
      );
      if (!gate.allowed) {
        const authState = await consumeStateForErrorNotice();
        return buildLoginErrorRedirectResponse({
          request,
          config,
          message: gate.retryAfter
            ? t("server.tooManyAttemptsWithRetry", {
                seconds: gate.retryAfter,
              })
            : t("server.tooManyAttempts"),
          redirectUri: authState?.redirect_uri,
          persistNotice: !!authState,
          extraSetCookies: resolveFlowClearCookies(),
        });
      }

      try {
        const resolved = await oidcAuthService.resolveCallback({
          request,
          providerId: params.providerId,
          code,
          state,
          flowToken,
        });
        const linkedTotpName = await resolveTotpCredentialName(
          resolved.binding.totp_id,
        );
        const credentialName =
          resolved.profile.display_name ||
          resolved.profile.email ||
          resolved.provider.name ||
          "External Account";
        const loginResult = await handleLoginSuccess({
          config,
          request,
          clientIp,
          userAgent,
          authMethod: "OIDC",
          authProviderName: resolved.provider.name,
          credentialId: resolved.binding.id,
          credentialName,
          ...(linkedTotpName ? { linkedTotpName } : {}),
          rememberMe: resolved.state.remember_me,
          set,
          totpId: resolved.binding.totp_id,
          redirectTo: resolved.state.redirect_uri,
        });
        const redirectTo =
          typeof loginResult.data?.redirect_to === "string"
            ? loginResult.data.redirect_to
            : "/";
        return buildRedirectResponse(
          redirectTo,
          set.headers,
          resolveFlowClearCookies(),
        );
      } catch (error) {
        const message = getErrorMessage(error, t("server.oidc.loginFailed"));
        if (message === OIDC_CALLBACK_STATE_EXPIRED_MESSAGE) {
          return buildLoginErrorRedirectResponse({
            request,
            config,
            message,
            persistNotice: false,
            extraSetCookies: resolveFlowClearCookies(),
          });
        }
        if (isOidcOperationAbortedError(error)) {
          return buildLoginErrorRedirectResponse({
            request,
            config,
            message: t("server.oidc.operationAborted"),
            persistNotice: true,
            extraSetCookies: resolveFlowClearCookies(),
          });
        }
        const failure = await registerAuthFailure({
          clientIp,
          userAgent,
          method: "OIDC",
          credentialName: providerName,
        });
        return buildLoginErrorRedirectResponse({
          request,
          config,
          message: t("server.oidc.loginFailedRetryAfter", {
            message,
            seconds: failure.retryAfter,
          }),
          persistNotice: true,
          extraSetCookies: resolveFlowClearCookies(),
        });
      }
    },
    withRouteDoc("处理外部登录回调", {
      params: t.Object({
        providerId: t.String(),
      }),
      query: t.Object({
        code: t.Optional(t.String()),
        state: t.Optional(t.String()),
        error: t.Optional(t.String()),
        error_description: t.Optional(t.String()),
      }),
    }),
  );
