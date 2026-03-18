import { Elysia, t } from "elysia";
import { configManager } from "../lib/redis";
import { verifySync } from "otplib";
import {
  buildPasskeyBindInfo,
  getRpInfo,
  handleLoginSuccess,
} from "../lib/auth-utils";
import {
  applyAuthResponseHeaders,
  hasNormalAccessContext,
  resolveAuthAccess,
  resolveRequestedAccessMode,
} from "../lib/auth-access";
import { authMobilitySessionManager } from "../lib/auth-mobility-session";
import { buildClientInfo, getClientIp } from "../lib/auth-request";
import { passkeyRoutes } from "./auth/passkey";
import { whitelistManager } from "../lib/whitelist-manager";
import { authLogManager } from "../lib/auth-log";
import { loginBackoffService } from "../lib/login-backoff";
import { recentAuthIPsManager } from "../lib/recent-auth-ips";
import { scanDetector } from "../lib/scan-detector";
import { ipLocationService } from "../lib/ip-location";
import {
  buildFnosShareSessionClearCookie,
  buildSessionClearCookie,
} from "../lib/session-cookie";
import { fnosShareBypassService } from "../lib/fnos-share-bypass";
import { captchaService } from "../lib/captcha";
import {
  resolveCookieDomain,
  resolveSafeRedirectUri,
} from "../lib/subdomain-mode";

const buildPasskeyStatus = async (request: Request) => {
  const passkeys = await configManager.getPasskeys();
  const rpInfo = await getRpInfo(request);
  return {
    available: passkeys.length > 0,
    mode: rpInfo.mode,
    rp_id: rpInfo.rpID,
  };
};

export const authRoutes = new Elysia({ prefix: "/api/auth" })
  .get("/bootstrap", async ({ request, set }) => {
    const clientIp = getClientIp(request);
    const config = await configManager.getConfig();
    ipLocationService.ensureEnqueued(clientIp).catch((error) => {
      console.error("[auth][bootstrap] failed to enqueue ip lookup:", error);
    });
    const [auth, client, captcha, passkey] = await Promise.all([
      resolveAuthAccess(request, clientIp),
      Promise.resolve(buildClientInfo(clientIp)),
      captchaService.getPublicSettings(),
      buildPasskeyStatus(request),
    ]);

    applyAuthResponseHeaders(set, auth);
    const redirectTo = auth.authorized
      ? resolveSafeRedirectUri({
          config,
          request,
          redirectUri: new URL(request.url).searchParams.get("redirect_uri"),
        })
      : null;

    return {
      success: true,
      data: {
        auth: {
          authenticated: auth.authorized,
          message: auth.message,
        },
        client,
        captcha,
        passkey,
        redirect_to: redirectTo || undefined,
      },
    };
  })
  .get("/session", async ({ request, set }) => {
    const clientIp = getClientIp(request);
    const auth = await resolveAuthAccess(request, clientIp);
    applyAuthResponseHeaders(set, auth);

    if (!auth.authorized) {
      set.status = 401;
      return { success: false, message: auth.message };
    }

    ipLocationService.ensureEnqueued(clientIp).catch((error) => {
      console.error("[auth][session] failed to enqueue ip lookup:", error);
    });
    const [client, passkey] = await Promise.all([
      Promise.resolve(buildClientInfo(clientIp)),
      buildPasskeyStatus(request),
    ]);

    return {
      success: true,
      data: {
        auth: {
          authenticated: true,
          message: auth.message,
        },
        client,
        passkey,
      },
    };
  })
  .get("/captcha/config", async () => {
    const settings = await captchaService.getPublicSettings();
    return { success: true, data: settings };
  })
  .get("/challenge", async ({ set }) => {
    try {
      return await captchaService.createChallenge();
    } catch (error: any) {
      set.status = 503;
      return {
        success: false,
        message: error?.message || "验证码服务暂时不可用",
      };
    }
  })
  .get("/ip", async ({ request }) => {
    const clientIp = getClientIp(request);
    const client = buildClientInfo(clientIp);

    return {
      success: true,
      data: {
        ip: client.ip,
      },
    };
  })
  .get("/ip/location", async ({ request }) => {
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
  })
  .post(
    "/login",
    async ({ body, set, request }) => {
      const config = await configManager.getConfig();
      const clientIp = getClientIp(request);
      const gate = await loginBackoffService.ensureNotBlocked(clientIp);
      if (!gate.allowed) {
        set.status = 429;
        if (gate.retryAfter)
          set.headers["Retry-After"] = String(gate.retryAfter);
        return {
          success: false,
          message: "尝试过于频繁，请稍后重试",
          retryAfter: gate.retryAfter,
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
        return { success: false, message: "服务器尚未配置登录凭据" };
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
        await authLogManager.recordLog({
          type: "login",
          method: "TOTP",
          ip: clientIp,
          userAgent,
          success: false,
          credentialName: "! Unknown TOTP",
        });
        const rf = await loginBackoffService.registerFailure(clientIp);
        set.status = 429;
        set.headers["Retry-After"] = String(rf.retryAfter);
        return {
          success: false,
          message: `验证码不正确，请在 ${rf.retryAfter} 秒后重试`,
          retryAfter: rf.retryAfter,
        };
      }
      const passkeyInfo = await buildPasskeyBindInfo(matchedTotpId);
      const userAgent = request.headers.get("user-agent") || "Unknown";
      const credentialName =
        totpCredentials.find((t) => t.id === matchedTotpId)?.comment ||
        "Unknown TOTP";

      await loginBackoffService.reset(clientIp);
      const redirectTo = resolveSafeRedirectUri({
        config,
        request,
        redirectUri: body.redirect_uri,
      });
      return await handleLoginSuccess({
        config,
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
    {
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
    },
  )
  .use(passkeyRoutes)
  .get("/logout", async ({ request, set }) => {
    const config = await configManager.getConfig();
    const cookieDomain = resolveCookieDomain(config);
    const { sessionId } = authMobilitySessionManager.inspectRequest(request);
    let loginIpFromSession: string | null = null;
    if (sessionId) {
      const session = await configManager.getSession(sessionId);
      loginIpFromSession = session?.ip || null;
      await authMobilitySessionManager.destroySession(sessionId);
      await configManager.deleteSession(sessionId);
    }

    const clientIp = getClientIp(request);
    const userAgent = request.headers.get("user-agent") || "Unknown";
    if (!sessionId) {
      await whitelistManager.removeRecordsByIP(
        loginIpFromSession || clientIp,
        "auto",
      );
    }

    await authLogManager.recordLog({
      type: "logout",
      ip: clientIp,
      userAgent,
      success: true,
    });

    const headers = new Headers({
      Location: "/",
    });
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
  })
  .head("/preflight", async ({ request }) => {
    const clientIp = getClientIp(request);
    const forwardedPath = request.headers.get("x-forwarded-path") || "";
    const headers = new Headers();
    const accessMode = resolveRequestedAccessMode(request);

    try {
      const config = await configManager.getConfig();
      let shareDecision: Awaited<
        ReturnType<typeof fnosShareBypassService.resolvePreflight>
      > | null = null;

      if (
        accessMode === "strict_whitelist" &&
        !(await whitelistManager.hasValidIP(clientIp))
      ) {
        headers.set("X-Option", "Deny");
      } else if (
        !(await hasNormalAccessContext(request, clientIp, accessMode))
      ) {
        shareDecision = await fnosShareBypassService.resolvePreflight(request);
        if (shareDecision.redirectLocation) {
          headers.set(
            "X-Reauth-Redirect-Location",
            shareDecision.redirectLocation,
          );
        }
      }

      if (config.run_type !== 0) {
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
        xForwardedFor: request.headers.get("x-forwarded-for"),
        xRealIp: request.headers.get("x-real-ip"),
      });
    }

    return new Response(null, {
      status: 204,
      headers,
    });
  })
  .get("/verify", async ({ request, set }) => {
    const auth = await resolveAuthAccess(request);
    applyAuthResponseHeaders(set, auth);
    if (auth.authorized) {
      return { success: true, message: auth.message };
    }

    set.status = 401;
    return { success: false, message: auth.message };
  });
