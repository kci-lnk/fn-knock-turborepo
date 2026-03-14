import { Elysia, t } from "elysia";
import { configManager } from "../lib/redis";
import { verifySync } from "otplib";
import { ipLocationService } from "../lib/ip-location";
import {
  buildPasskeyBindInfo,
  handleLoginSuccess,
} from "../lib/auth-utils";
import { authMobilitySessionManager } from "../lib/auth-mobility-session";
import { passkeyRoutes } from "./auth/passkey";
import { whitelistManager } from "../lib/whitelist-manager";
import { authLogManager } from "../lib/auth-log";
import { loginBackoffService } from "../lib/login-backoff";
import { recentAuthIPsManager } from "../lib/recent-auth-ips";
import { scanDetector } from "../lib/scan-detector";
import {
  buildFnosShareSessionClearCookie,
  buildSessionClearCookie,
} from "../lib/session-cookie";
import { fnosShareBypassService } from "../lib/fnos-share-bypass";
import { captchaService } from "../lib/captcha";
const getClientIp = (request: Request): string =>
  request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || "::1";

const hasNormalAccessContext = async (
  request: Request,
  clientIp: string,
): Promise<boolean> => {
  if (await whitelistManager.hasValidIP(clientIp)) {
    return true;
  }

  const identity = authMobilitySessionManager.inspectRequest(request);
  if (identity.sessionId) {
    return configManager.isValidSession(identity.sessionId);
  }

  return false;
};

export const authRoutes = new Elysia({ prefix: "/api/auth" })
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
    let ipLocationStr = "";

    try {
      const ipAddr = clientIp === '::1' ? '127.0.0.1' : clientIp;
      const ipInfo = await ipLocationService.getIpLocation(ipAddr);
      if (ipInfo) {
        ipLocationStr = ipInfo.raw;
      }
    } catch (err) {
      console.error("Failed to query IP location:", err);
    }

    return {
      success: true,
      data: {
        ip: clientIp,
        location: ipLocationStr
      }
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
        if (gate.retryAfter) set.headers["Retry-After"] = String(gate.retryAfter);
        return { success: false, message: "尝试过于频繁，请稍后重试", retryAfter: gate.retryAfter };
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
        return { success: false, message: `验证码不正确，请在 ${rf.retryAfter} 秒后重试`, retryAfter: rf.retryAfter };
      }
      const passkeyInfo = await buildPasskeyBindInfo(matchedTotpId);
      const userAgent = request.headers.get("user-agent") || "Unknown";
      const credentialName = totpCredentials.find((t) => t.id === matchedTotpId)?.comment || "Unknown TOTP";

      await loginBackoffService.reset(clientIp);
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
      }),
    },
  )
  .use(passkeyRoutes)
  .get("/logout", async ({ request, set }) => {
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
      await whitelistManager.removeRecordsByIP(loginIpFromSession || clientIp, 'auto');
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
    headers.append("Set-Cookie", buildSessionClearCookie());
    headers.append("Set-Cookie", buildFnosShareSessionClearCookie());
    return new Response("", {
      status: 302,
      headers,
    });
  })
  .head("/preflight", async ({ request }) => {
    const clientIp = getClientIp(request);
    const forwardedPath = request.headers.get("x-forwarded-path") || "";
    const headers = new Headers();

    try {
      const config = await configManager.getConfig();
      let shareDecision: Awaited<
        ReturnType<typeof fnosShareBypassService.resolvePreflight>
      > | null = null;

      if (!(await hasNormalAccessContext(request, clientIp))) {
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
    const clientIp = getClientIp(request);
    const isWhitelisted = await whitelistManager.hasValidIP(clientIp);
    if (isWhitelisted) {
      await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
      await recentAuthIPsManager.recordVerified(clientIp);
      return { success: true, message: "Authorized by IP whitelist" };
    }

    const restored = await authMobilitySessionManager.tryRestoreAccess(request, clientIp);
    if (restored.success) {
      await recentAuthIPsManager.recordVerified(clientIp);
      return { success: true, message: restored.message || "Authorized" };
    }

    const shareAuth = await fnosShareBypassService.authorize(request);
    const [shareCookie] = shareAuth.setCookies ?? [];
    if (shareCookie) {
      set.headers["Set-Cookie"] = shareCookie;
    }
    if ("responseHeaders" in shareAuth && shareAuth.responseHeaders) {
      for (const [key, value] of Object.entries(shareAuth.responseHeaders)) {
        set.headers[key] = value;
      }
    }
    if (shareAuth.authorized) {
      return { success: true, message: "Authorized by fnos share link" };
    }

    set.status = 401;
    return { success: false, message: "Unauthorized" };
  });
