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

const resolveProviderErrorMessage = (error: string | undefined) => {
  switch (
    String(error || "")
      .trim()
      .toLowerCase()
  ) {
    case "access_denied":
      return "你取消了外部登录授权，或授权请求被提供商拒绝。";
    case "temporarily_unavailable":
      return "外部登录服务暂时不可用，请稍后重试。";
    case "server_error":
      return "外部登录提供商返回服务错误，请稍后重试。";
    case "invalid_scope":
      return "外部登录权限范围配置不正确，请联系管理员检查提供商配置。";
    case "invalid_request":
    case "unauthorized_client":
    case "unsupported_response_type":
      return "外部登录请求被提供商拒绝，请检查外部登录配置后重试。";
    default:
      return "外部登录未完成，请重新发起登录。";
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
<html lang="zh-CN">
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
) => {
  const actions = invite.providers
    .map((provider) => {
      const url = new URL(request.url);
      url.search = "";
      url.searchParams.set("token", token);
      url.searchParams.set("provider_id", provider.id);
      return `<a href="${escapeHtml(`${url.pathname}${url.search}`)}">使用 ${escapeHtml(provider.name)} 绑定</a>`;
    })
    .join("");

  return buildBindHtmlResponse(
    200,
    "选择外部账号提供商",
    `将外部账号绑定到 ${invite.totp.comment || "TOTP"}。`,
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
    async ({ query, set }) => {
      const token = query.token?.trim();
      if (!token) {
        set.status = 400;
        return { success: false, message: "绑定邀请链接无效" };
      }
      const invite = await oidcAuthService.inspectInvite(token);
      if (!invite) {
        set.status = 404;
        return { success: false, message: "绑定邀请链接已失效" };
      }
      return { success: true, data: invite };
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
      const token = query.token?.trim();
      if (!token) {
        return buildBindHtmlResponse(
          400,
          "绑定邀请链接无效",
          "链接缺少 token。",
        );
      }

      const invite = await oidcAuthService.inspectInvite(token);
      if (!invite) {
        return buildBindHtmlResponse(
          404,
          "绑定邀请链接已失效",
          "该邀请不存在、已过期或已经被使用。",
        );
      }
      if (invite.providers.length === 0) {
        return buildBindHtmlResponse(
          404,
          "没有可用的外部登录提供商",
          "该邀请当前没有可用于绑定的外部账号提供商。",
        );
      }

      const selectedProviderId =
        query.provider_id?.trim() ||
        invite.provider_id ||
        (invite.providers.length === 1 ? invite.providers[0]?.id : "");

      if (!selectedProviderId) {
        return buildBindProviderSelectionResponse(request, token, invite);
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
        const config = await configManager.getConfig();
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
          "外部账号绑定失败",
          error instanceof Error ? error.message : "无法发起外部账号绑定。",
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
        const config = await configManager.getConfig();
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
          message: error instanceof Error ? error.message : "发起外部登录失败",
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
          message: resolveProviderErrorMessage(query.error),
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
          message: "外部登录回调缺少必要参数，请重新发起登录。",
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
            ? `尝试过于频繁，请在 ${gate.retryAfter} 秒后重试`
            : "尝试过于频繁，请稍后重试",
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
        const message = getErrorMessage(error, "外部登录失败");
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
            message: "外部登录请求已中断，请重新发起登录。",
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
          message: `${message}，请在 ${failure.retryAfter} 秒后重试`,
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
