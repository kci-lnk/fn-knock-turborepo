import { Elysia, t } from "elysia";
import { configManager } from "../lib/redis";
import { verifySync } from "otplib";
import {
  buildPasskeyBindInfo,
  getRpInfo,
  handleLoginSuccess,
} from "../lib/auth-utils";
import {
  applyNoStoreHeaders,
  applyAuthResponseHeaders,
  hasWhitelistAccess,
  hasNormalAccessContext,
  reliesOnBrowserSessionCookie,
  resolveAuthAccess,
  resolveRequestedAccessMode,
} from "../lib/auth-access";
import { authMobilitySessionManager } from "../lib/auth-mobility-session";
import { buildClientInfo, getClientIp } from "../lib/auth-request";
import { passkeyRoutes } from "./auth/passkey";
import { oidcRoutes } from "./auth/oidc";
import { oidcAuthService } from "../lib/auth/oidc/service";
import { whitelistManager } from "../lib/whitelist-manager";
import { loginBackoffService } from "../lib/login-backoff";
import { recentAuthIPsManager } from "../lib/recent-auth-ips";
import { scanDetector } from "../lib/scan-detector";
import { ipLocationService } from "../lib/ip-location";
import {
  normalizeAuthFailureTrackingIp,
  registerAuthFailure,
} from "../lib/auth-failure";
import { revokeCustomPostLoginIpGrant } from "../lib/post-login-ip-grant";
import { scheduleSyncReverseProxyTrustedIPs } from "../lib/reverse-proxy-trusted-ips";
import { emitLogoutEvent } from "../lib/system-events/helpers";
import {
  buildFnosShareSessionClearCookie,
  buildOidcLoginErrorClearCookie,
  OIDC_LOGIN_ERROR_COOKIE_NAME,
  readCookieValue,
  buildSessionClearCookie,
} from "../lib/session-cookie";
import { fnosShareBypassService } from "../lib/fnos-share-bypass";
import { captchaService } from "../lib/captcha";
import {
  canBrowserSessionReachRedirectUri,
  resolveCookieDomain,
  resolvePublicAuthBaseUrl,
  resolveRequestHostname,
  resolveSharedAuthLoginRedirect,
  resolveSafeRedirectUri,
} from "../lib/subdomain-mode";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { createRequestTranslator } from "../lib/i18n";
import { normalizeLocaleConfig } from "../../../../packages/i18n/src";

const buildPasskeyStatus = async (request: Request) => {
  const config = await configManager.getConfig();
  const passkeys = await configManager.getPasskeys();
  const rpInfo = await getRpInfo(request);
  const requestHost = resolveRequestHostname(request);
  const parentRpId = rpInfo.rpID.trim().toLowerCase();
  const sharedAuthBaseUrl = resolvePublicAuthBaseUrl(config);
  let sharedAuthHost = "";
  if (sharedAuthBaseUrl) {
    try {
      sharedAuthHost = new URL(sharedAuthBaseUrl).hostname.toLowerCase();
    } catch {
      sharedAuthHost = "";
    }
  }
  const isPasskeyAvailableOnCurrentHost =
    rpInfo.mode === "parent_domain"
      ? !!parentRpId &&
        (requestHost === parentRpId || requestHost.endsWith(`.${parentRpId}`))
      : !sharedAuthHost || requestHost === sharedAuthHost;
  return {
    available: passkeys.length > 0 && isPasskeyAvailableOnCurrentHost,
    mode: rpInfo.mode,
    rp_id: rpInfo.rpID,
  };
};

const buildOidcStatus = async () => ({
  providers: await oidcAuthService.listPublicProviders(),
});

const resolveAuthUiBasePrefix = (request: Request): string => {
  const resolveBasePrefixFromPathname = (pathname: string): string => {
    if (pathname === "/__auth__" || pathname.startsWith("/__auth__/")) {
      return "/__auth__";
    }
    if (pathname === "/auth" || pathname.startsWith("/auth/")) {
      return "/auth";
    }
    return "";
  };

  const parsePathname = (value: string | null): string => {
    if (!value) return "";
    try {
      return new URL(value, "http://127.0.0.1").pathname;
    } catch {
      return "";
    }
  };

  const candidates = [
    parsePathname(request.url),
    parsePathname(request.headers.get("x-forwarded-path")),
    parsePathname(request.headers.get("referer")),
  ];

  for (const pathname of candidates) {
    const basePrefix = resolveBasePrefixFromPathname(pathname);
    if (basePrefix) return basePrefix;
  }

  return "";
};

const buildPostLogoutLocation = (request: Request): string => {
  const basePrefix = resolveAuthUiBasePrefix(request);
  const params = new URLSearchParams({ logged_out: "1" });
  return `${basePrefix}/login?${params.toString()}`;
};

const appendSetCookieHeader = (
  set: { headers: Record<string, unknown> },
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

export const authRoutes = new Elysia({ prefix: "/api/auth", tags: ["Auth"] })
  .onBeforeHandle(({ set }) => {
    applyNoStoreHeaders(set.headers);
  })
  .get(
    "/bootstrap",
    async ({ request, set }) => {
      const clientIp = getClientIp(request);
      const config = await configManager.getConfig();
      ipLocationService.ensureEnqueued(clientIp).catch((error) => {
        console.error("[auth][bootstrap] failed to enqueue ip lookup:", error);
      });
      const [auth, client, captcha, passkey, oidc] = await Promise.all([
        resolveAuthAccess(request, clientIp),
        Promise.resolve(buildClientInfo(clientIp)),
        captchaService.getPublicSettings(),
        buildPasskeyStatus(request),
        buildOidcStatus(),
      ]);

      applyAuthResponseHeaders(set, auth);
      const requestedRedirectUri = new URL(request.url).searchParams.get(
        "redirect_uri",
      );
      const oidcLoginErrorTicket = readCookieValue(
        request.headers.get("cookie"),
        OIDC_LOGIN_ERROR_COOKIE_NAME,
      );
      const redirectTo = auth.authorized
        ? resolveSafeRedirectUri({
            config,
            request,
            redirectUri: requestedRedirectUri,
          })
        : resolveSharedAuthLoginRedirect({
            config,
            request,
            redirectUri: requestedRedirectUri,
          });
      const reachableRedirectTo =
        auth.authorized &&
        redirectTo &&
        reliesOnBrowserSessionCookie(auth.grantType) &&
        !canBrowserSessionReachRedirectUri({
          config,
          request,
          redirectUri: redirectTo,
        })
          ? null
          : redirectTo;
      const oidcLoginError =
        await oidcAuthService.consumeLoginErrorNotice(oidcLoginErrorTicket);

      if (oidcLoginErrorTicket) {
        appendSetCookieHeader(
          set,
          buildOidcLoginErrorClearCookie({
            domain: resolveCookieDomain(config, request),
            path: resolveAuthUiBasePrefix(request) || "/",
          }),
        );
      }

      return {
        success: true,
        data: {
          locale: normalizeLocaleConfig(config.locale),
          auth: {
            authenticated: auth.authorized,
            message: auth.message,
            grant_type: auth.grantType,
          },
          client,
          captcha,
          passkey,
          oidc: {
            ...oidc,
            ...(oidcLoginError ? { login_error: oidcLoginError } : {}),
          },
          redirect_to: reachableRedirectTo || undefined,
        },
      };
    },
    routeDoc("获取认证页引导信息"),
  )
  .get(
    "/session",
    async ({ request, set }) => {
      const clientIp = getClientIp(request);
      const config = await configManager.getConfig();
      const auth = await resolveAuthAccess(request, clientIp);
      applyAuthResponseHeaders(set, auth);

      if (!auth.authorized) {
        set.status = 401;
        return { success: false, message: auth.message };
      }

      ipLocationService.ensureEnqueued(clientIp).catch((error) => {
        console.error("[auth][session] failed to enqueue ip lookup:", error);
      });
      const [client, passkey, oidc] = await Promise.all([
        Promise.resolve(buildClientInfo(clientIp)),
        buildPasskeyStatus(request),
        buildOidcStatus(),
      ]);

      return {
        success: true,
        data: {
          locale: normalizeLocaleConfig(config.locale),
          auth: {
            authenticated: true,
            message: auth.message,
            grant_type: auth.grantType,
          },
          client,
          passkey,
          oidc,
        },
      };
    },
    routeDoc("获取当前认证会话"),
  )
  .get(
    "/captcha/config",
    async ({ set }) => {
      const settings = await captchaService.getPublicSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取验证码公开配置"),
  )
  .get(
    "/challenge",
    async ({ set, request }) => {
      try {
        return await captchaService.createChallenge();
      } catch (error: any) {
        const config = await configManager.getConfig();
        const { t } = createRequestTranslator(request, config.locale);
        set.status = 503;
        return {
          success: false,
          message: error?.message || t("server.captchaUnavailable"),
        };
      }
    },
    routeDoc("创建登录验证码挑战"),
  )
  .get(
    "/ip",
    async ({ request, set }) => {
      const clientIp = getClientIp(request);
      const client = buildClientInfo(clientIp);

      return {
        success: true,
        data: {
          ip: client.ip,
        },
      };
    },
    routeDoc("获取当前客户端 IP"),
  )
  .get(
    "/ip/location",
    async ({ request, set }) => {
      const clientIp = getClientIp(request);
      const snapshot = await ipLocationService.ensureEnqueued(clientIp);

      return {
        success: true,
        data: {
          ip: clientIp,
          location: snapshot.location,
          status: snapshot.status,
          attempts: snapshot.attempts,
          maxAttempts: snapshot.maxAttempts,
          error: snapshot.error,
        },
      };
    },
    routeDoc("获取当前客户端 IP 属地"),
  )
  .post(
    "/login",
    async ({ body, set, request }) => {
      const config = await configManager.getConfig();
      const { t } = createRequestTranslator(request, config.locale);
      const clientIp = getClientIp(request);
      const gate = await loginBackoffService.ensureNotBlocked(
        normalizeAuthFailureTrackingIp(clientIp),
      );
      if (!gate.allowed) {
        set.status = 429;
        if (gate.retryAfter)
          set.headers["Retry-After"] = String(gate.retryAfter);
        return {
          success: false,
          message: gate.retryAfter
            ? t("server.tooManyAttemptsWithRetry", {
                seconds: gate.retryAfter,
              })
            : t("server.tooManyAttempts"),
          retryAfter: gate.retryAfter,
          blockedUntil: gate.blockedUntil,
        };
      }
      try {
        await captchaService.verify(body.captcha, { clientIp });
      } catch (e: any) {
        set.status = 400;
        return {
          success: false,
          message: e.message,
        };
      }
      const totpCredentials = await configManager.getTOTPCredentials();
      if (totpCredentials.length === 0) {
        set.status = 400;
        return { success: false, message: t("server.loginCredentialMissing") };
      }

      let matchedTotpId: string | null = null;
      for (const totp of totpCredentials) {
        const { valid } = verifySync({
          strategy: "totp",
          token: body.token,
          secret: totp.secret,
        });
        if (valid) {
          matchedTotpId = totp.id;
          break;
        }
      }

      if (!matchedTotpId) {
        const userAgent = request.headers.get("user-agent") || "Unknown";
        const rf = await registerAuthFailure({
          clientIp,
          userAgent,
          method: "TOTP",
          credentialName: "! Unknown TOTP",
        });
        set.status = 429;
        set.headers["Retry-After"] = String(rf.retryAfter);
        return {
          success: false,
          message: t("server.invalidOtpWithRetry", {
            seconds: rf.retryAfter,
          }),
          retryAfter: rf.retryAfter,
        };
      }
      const userAgent = request.headers.get("user-agent") || "Unknown";
      const credentialName =
        totpCredentials.find((t) => t.id === matchedTotpId)?.comment ||
        "Unknown TOTP";

      const redirectTo = resolveSafeRedirectUri({
        config,
        request,
        redirectUri: body.redirect_uri,
      });
      const passkeyInfo =
        config.auth_credential_settings?.passkey_bind_prompt_enabled === false
          ? undefined
          : await buildPasskeyBindInfo(matchedTotpId);
      return await handleLoginSuccess({
        config,
        request,
        clientIp,
        userAgent,
        authMethod: "TOTP",
        credentialId: matchedTotpId,
        credentialName,
        rememberMe: body.rememberMe,
        set,
        totpId: matchedTotpId,
        passkeyInfo,
        redirectTo,
      });
    },
    withRouteDoc("使用 TOTP 登录", {
      body: t.Object({
        token: t.String(),
        captcha: t.Union([
          t.Object({
            provider: t.Literal("pow"),
            proof: t.String(),
          }),
          t.Object({
            provider: t.Literal("turnstile"),
            token: t.String(),
          }),
        ]),
        rememberMe: t.Boolean(),
        redirect_uri: t.Optional(t.String()),
      }),
    }),
  )
  .use(passkeyRoutes)
  .use(oidcRoutes)
  .get(
    "/logout",
    async ({ request, set }) => {
      const config = await configManager.getConfig();
      const cookieDomain = resolveCookieDomain(config, request);
      const { sessionId } = authMobilitySessionManager.inspectRequest(request);
      let session: Awaited<ReturnType<typeof configManager.getSession>> = null;
      let loginIpFromSession: string | null = null;
      if (sessionId) {
        session = await configManager.getSession(sessionId);
        loginIpFromSession = session?.ip || null;
        await authMobilitySessionManager.destroySession(sessionId);
        await configManager.deleteSession(sessionId);
      }

      const clientIp = getClientIp(request);
      if (!sessionId) {
        await whitelistManager.removeRecordsByIP(
          loginIpFromSession || clientIp,
          "auto",
        );
      } else {
        await revokeCustomPostLoginIpGrant(
          session,
          config,
          loginIpFromSession || clientIp,
        );
      }

      scheduleSyncReverseProxyTrustedIPs({ reason: "logout" });
      if (sessionId && session) {
        await emitLogoutEvent({
          sessionId,
          authMethod: session.method,
          credentialId: session.credentialId,
          credentialName: session.credentialName,
          ...(session.linkedTotpName
            ? { linkedTotpName: session.linkedTotpName }
            : {}),
          ...(session.comment ? { sessionComment: session.comment } : {}),
          ip: session.ip,
          ...(session.ipLocation ? { ipLocation: session.ipLocation } : {}),
          userAgent: session.userAgent,
          ...(session.loginTime ? { loginTime: session.loginTime } : {}),
          logoutSource: "user_logout",
        });
      }

      const headers = new Headers({
        Location: buildPostLogoutLocation(request),
      });
      applyNoStoreHeaders(headers);
      headers.append(
        "Set-Cookie",
        buildSessionClearCookie({ domain: cookieDomain }),
      );
      headers.append(
        "Set-Cookie",
        buildFnosShareSessionClearCookie({ domain: cookieDomain }),
      );
      return new Response("", {
        status: 302,
        headers,
      });
    },
    routeDoc("退出当前认证会话"),
  )
  .head(
    "/preflight",
    async ({ request }) => {
      const clientIp = getClientIp(request);
      const forwardedPath = request.headers.get("x-forwarded-path") || "";
      const headers = new Headers();
      applyNoStoreHeaders(headers);
      const accessMode = resolveRequestedAccessMode(request);
      let config: Awaited<ReturnType<typeof configManager.getConfig>> | null =
        null;

      try {
        config = await configManager.getConfig();
        let shareDecision: Awaited<
          ReturnType<typeof fnosShareBypassService.resolvePreflight>
        > | null = null;

        if (
          accessMode === "strict_whitelist" &&
          !(await hasWhitelistAccess(clientIp))
        ) {
          headers.set("X-Option", "Deny");
        } else if (
          !(await hasNormalAccessContext(request, clientIp, accessMode))
        ) {
          shareDecision =
            await fnosShareBypassService.resolvePreflight(request);
          if (shareDecision.redirectLocation) {
            headers.set(
              "X-Reauth-Redirect-Location",
              shareDecision.redirectLocation,
            );
          }
        }

        const isScannerExemptRequest = scanDetector.isRequestExemptFromScan(
          request,
          config,
        );

        if (config.run_type !== 0 && !isScannerExemptRequest) {
          const isBlacklisted = await scanDetector.isBlacklisted(clientIp);
          if (isBlacklisted) {
            headers.set("X-Option", "Deny");
          } else {
            const isRecent = await recentAuthIPsManager.isActive(clientIp);
            if (
              !isRecent &&
              !shareDecision?.handled &&
              forwardedPath &&
              !(await scanDetector.isCommonPath(forwardedPath))
            ) {
              await scanDetector.recordUncommonPath(clientIp, forwardedPath);
            }
          }
        }
      } catch (error) {
        console.error("[auth][preflight] failed:", {
          error,
          clientIp,
          forwardedPath,
          edgeClientIPEnabled:
            config?.run_type === 3 &&
            config.subdomain_mode?.edge_client_ip_enabled === true,
          aliyunESAEnabled:
            config?.run_type === 3 &&
            config.subdomain_mode?.aliyun_esa_enabled === true,
          tencentEdgeOneEnabled:
            config?.run_type === 3 &&
            config.subdomain_mode?.tencent_edgeone_enabled === true,
          eoConnectingIp: request.headers.get("eo-connecting-ip"),
          aliRealClientIp: request.headers.get("ali-real-client-ip"),
          xForwardedFor: request.headers.get("x-forwarded-for"),
          xRealIp: request.headers.get("x-real-ip"),
        });
      }

      return new Response(null, {
        status: 204,
        headers,
      });
    },
    routeDoc("认证反代预检"),
  )
  .get(
    "/verify",
    async ({ request, set }) => {
      const clientIp = getClientIp(request);
      const auth = await resolveAuthAccess(request, clientIp);
      applyAuthResponseHeaders(set, auth);

      if (auth.authorized) {
        return { success: true, message: auth.message };
      }

      set.status = 401;
      return { success: false, message: auth.message };
    },
    routeDoc("校验当前认证状态"),
  );
