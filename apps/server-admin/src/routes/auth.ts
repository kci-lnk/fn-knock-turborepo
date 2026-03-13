import { Elysia, t } from "elysia";
import { configManager } from "../lib/redis";
import { verifySync } from "otplib";
import { randomBytes, createHmac, createHash } from "node:crypto";
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
import { getRequiredEnv } from "../lib/env";
import { safeEqualString } from "../lib/security";
import { buildSessionClearCookie } from "../lib/session-cookie";

const ALTCHA_HMAC_KEY = getRequiredEnv("ALTCHA_HMAC_KEY");
const MAX_NUMBER = 100000;
const getClientIp = (request: Request): string =>
  request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || "::1";

export const authRoutes = new Elysia({ prefix: "/api/auth" })
  .get("/challenge", () => {
    const salt = randomBytes(12).toString("hex");
    // Expires in 5 minutes
    const expires = Math.floor(Date.now() / 1000) + 300;
    const saltWithParams = `${salt}?expires=${expires}`;

    const secret_number = Math.floor(Math.random() * MAX_NUMBER);

    const hash = createHash("sha256");
    hash.update(saltWithParams + secret_number.toString());
    const challenge = hash.digest("hex");

    const hmac = createHmac("sha256", ALTCHA_HMAC_KEY);
    hmac.update(challenge);
    const signature = hmac.digest("hex");

    return {
      algorithm: "SHA-256",
      challenge,
      maxnumber: MAX_NUMBER,
      salt: saltWithParams,
      signature,
    };
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
        const payloadDecoded = Buffer.from(body.altcha, "base64").toString(
          "utf-8",
        );
        const data = JSON.parse(payloadDecoded);

        if (data.algorithm !== "SHA-256") {
          throw new Error("Invalid algorithm");
        }

        const expectedChallenge = createHash("sha256")
          .update(data.salt + data.number.toString())
          .digest("hex");
        if (!safeEqualString(String(data.challenge || "").toLowerCase(), expectedChallenge)) {
          throw new Error("Invalid challenge");
        }

        const expectedSignature = createHmac("sha256", ALTCHA_HMAC_KEY)
          .update(data.challenge)
          .digest("hex");
        if (!safeEqualString(String(data.signature || "").toLowerCase(), expectedSignature)) {
          throw new Error("Invalid signature");
        }

        const expiresMatch = data.salt.match(/expires=(\d+)/);
        if (expiresMatch) {
          const expires = parseInt(expiresMatch[1], 10);
          if (Date.now() / 1000 > expires) {
            throw new Error("Challenge expired");
          }
        }

        const isNewChallenge = await configManager.setNonceIfNotExists(
          data.challenge,
          86400,
        );
        if (!isNewChallenge) {
          throw new Error("Challenge has already been used");
        }
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
        altcha: t.String(),
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

    set.headers["Set-Cookie"] = buildSessionClearCookie();
    set.status = 302;
    set.headers["Location"] = "/";
    return "";
  })
  .head("/preflight", async ({ request }) => {
    const clientIp = getClientIp(request);
    const forwardedPath = request.headers.get("x-forwarded-path") || "";
    const headers = new Headers();

    try {
      const config = await configManager.getConfig();
      if (config.run_type !== 0) {
        const isBlacklisted = await scanDetector.isBlacklisted(clientIp);
        if (isBlacklisted) {
          headers.set("X-Option", "Deny");
        } else {
          const isRecent = await recentAuthIPsManager.isActive(clientIp);
          if (!isRecent && forwardedPath && !(await scanDetector.isCommonPath(forwardedPath))) {
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

    set.status = 401;
    return { success: false, message: "Unauthorized" };
  });
