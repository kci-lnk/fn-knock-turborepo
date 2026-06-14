import { Elysia, t } from "elysia";
import { configManager, type PasskeyCredential } from "../../lib/redis";
import {
  base64UrlToBuffer,
  buildPasskeyBindInfo,
  bufferToBase64Url,
  extractChallenge,
  getRpInfo,
  handleLoginSuccess,
  resolveTotpCredentialName,
} from "../../lib/auth-utils";
import { getClientIp } from "../../lib/auth-request";
import {
  normalizeAuthFailureTrackingIp,
  registerAuthFailure,
} from "../../lib/auth-failure";
import { applyNoStoreHeaders } from "../../lib/auth-access";
import { loginBackoffService } from "../../lib/login-backoff";
import { resolveSafeRedirectUri } from "../../lib/subdomain-mode";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import { createRequestTranslator } from "../../lib/i18n";

const RP_NAME = "fn-knock";
let simpleWebAuthnServerPromise:
  | Promise<typeof import("@simplewebauthn/server")>
  | undefined;

const loadSimpleWebAuthnServer = () => {
  simpleWebAuthnServerPromise ??= import("@simplewebauthn/server");
  return simpleWebAuthnServerPromise;
};

const getPasskeyRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

const parseCookieValue = (
  cookieHeader: string,
  name: string,
): string | null => {
  const segments = cookieHeader.split(";");
  let lastValue: string | null = null;

  for (const segment of segments) {
    const [rawKey, ...rest] = segment.split("=");
    if (!rawKey || rest.length === 0) continue;
    if (rawKey.trim() !== name) continue;
    const raw = rest.join("=").trim().replace(/^"|"$/g, "");
    if (!raw) continue;
    try {
      lastValue = decodeURIComponent(raw);
    } catch {
      lastValue = raw;
    }
  }

  return lastValue;
};

export const passkeyRoutes = new Elysia({
  prefix: "/passkey",
  tags: ["Auth - Passkey"],
})
  .onBeforeHandle(({ set }) => {
    applyNoStoreHeaders(set.headers);
  })
  .get(
    "/status",
    async ({ request, set }) => {
      const passkeys = await configManager.getPasskeys();
      const rpInfo = await getRpInfo(request);
      return {
        success: true,
        data: {
          available: passkeys.length > 0,
          mode: rpInfo.mode,
          rp_id: rpInfo.rpID,
        },
      };
    },
    routeDoc("获取 Passkey 登录状态"),
  )
  .post(
    "/auth/options",
    async ({ set, request }) => {
      const passkeys = await configManager.getPasskeys();
      if (passkeys.length === 0) {
        set.status = 404;
        return { success: false, message: "No passkey available" };
      }
      const { generateAuthenticationOptions } =
        await loadSimpleWebAuthnServer();
      const { rpID } = await getRpInfo(request);
      const allowCredentials = passkeys.map((passkey) => ({
        id: passkey.id,
        type: "public-key" as const,
        transports: passkey.transports as any,
      }));
      const options = await generateAuthenticationOptions({
        rpID,
        allowCredentials,
        userVerification: "preferred",
      });
      await configManager.setPasskeyChallenge(options.challenge, "auth");
      return { success: true, data: options };
    },
    routeDoc("创建 Passkey 登录选项"),
  )
  .post(
    "/auth/verify",
    async ({ body, set, request }) => {
      const { t } = await getPasskeyRouteTranslator(request);
      const { origin, rpID } = await getRpInfo(request);
      const clientIp = getClientIp(request);
      const userAgent = request.headers.get("user-agent") || "Unknown";
      const gate = await loginBackoffService.ensureNotBlocked(
        normalizeAuthFailureTrackingIp(clientIp),
      );

      if (!gate.allowed) {
        set.status = 429;
        if (gate.retryAfter) {
          set.headers["Retry-After"] = String(gate.retryAfter);
        }
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

      const credential = body.credential;
      const challenge = extractChallenge(credential?.response?.clientDataJSON);
      if (!challenge) {
        set.status = 400;
        return { success: false, message: "Invalid passkey response" };
      }
      const isChallengeValid = await configManager.consumePasskeyChallenge(
        challenge,
        "auth",
      );
      if (!isChallengeValid) {
        set.status = 400;
        return { success: false, message: "Passkey challenge expired" };
      }
      const passkeys = await configManager.getPasskeys();
      const matched = passkeys.find((passkey) => passkey.id === credential.id);

      if (!matched) {
        const failure = await registerAuthFailure({
          clientIp,
          userAgent,
          method: "PASSKEY",
          credentialName: "Unknown Passkey",
        });
        set.status = 429;
        set.headers["Retry-After"] = String(failure.retryAfter);
        return {
          success: false,
          message: t("server.passkeyRoutes.notFoundWithRetry", {
            seconds: failure.retryAfter,
          }),
          retryAfter: failure.retryAfter,
        };
      }
      const linkedTotpName = await resolveTotpCredentialName(matched.totpId);

      let verification;
      try {
        const { verifyAuthenticationResponse } =
          await loadSimpleWebAuthnServer();
        const storedCredential = {
          id: matched.id,
          publicKey: new Uint8Array(base64UrlToBuffer(matched.publicKey)),
          counter: matched.counter,
          transports: matched.transports as any,
        };
        verification = await verifyAuthenticationResponse({
          response: credential,
          expectedChallenge: challenge,
          expectedOrigin: origin,
          expectedRPID: rpID,
          credential: storedCredential,
          requireUserVerification: false,
        });
      } catch (error: any) {
        console.error("WebAuthn Verification Error:", error.message);
        const failure = await registerAuthFailure({
          clientIp,
          userAgent,
          method: "PASSKEY",
          credentialName: matched.deviceName,
          linkedTotpName,
        });
        set.status = 429;
        set.headers["Retry-After"] = String(failure.retryAfter);
        return {
          success: false,
          message: t("server.passkeyRoutes.verifyFailedWithRetry", {
            seconds: failure.retryAfter,
          }),
          retryAfter: failure.retryAfter,
        };
      }

      if (!verification?.verified) {
        const failure = await registerAuthFailure({
          clientIp,
          userAgent,
          method: "PASSKEY",
          credentialName: matched.deviceName,
          linkedTotpName,
        });
        set.status = 429;
        set.headers["Retry-After"] = String(failure.retryAfter);
        return {
          success: false,
          message: t("server.passkeyRoutes.verifyFailedWithRetry", {
            seconds: failure.retryAfter,
          }),
          retryAfter: failure.retryAfter,
        };
      }

      await configManager.updatePasskeyCounter(
        matched.id,
        verification.authenticationInfo.newCounter,
        new Date().toISOString(),
      );
      const config = await configManager.getConfig();
      const redirectTo = resolveSafeRedirectUri({
        config,
        request,
        redirectUri: body.redirect_uri,
      });
      return await handleLoginSuccess({
        config,
        request,
        clientIp,
        userAgent,
        authMethod: "PASSKEY",
        credentialId: matched.id,
        credentialName: matched.deviceName,
        ...(linkedTotpName ? { linkedTotpName } : {}),
        rememberMe: body.rememberMe,
        set,
        totpId: matched.totpId,
        redirectTo,
      });
    },
    withRouteDoc("校验 Passkey 登录", {
      body: t.Object({
        credential: t.Any(),
        rememberMe: t.Boolean(),
        redirect_uri: t.Optional(t.String()),
      }),
    }),
  )
  .post(
    "/bind-token",
    async ({ set, request }) => {
      const cookieHeader = request.headers.get("cookie") || "";
      const sessionId = parseCookieValue(
        cookieHeader,
        "x-go-reauth-proxy-session-id",
      );
      let totpId = "";
      if (sessionId) {
        const session = await configManager.getSession(sessionId);
        totpId = session?.totpId || "";
      }
      if (!totpId) {
        set.status = 401;
        return { success: false, message: "Unauthorized or missing TOTP ID" };
      }
      return { success: true, data: await buildPasskeyBindInfo(totpId) };
    },
    routeDoc("获取 Passkey 绑定令牌信息"),
  )
  .post(
    "/register/options",
    async ({ body, set, request }) => {
      const { t } = await getPasskeyRouteTranslator(request);
      const isTokenValid = await configManager.isPasskeyBindTokenValid(
        body.token,
      );
      if (!isTokenValid) {
        set.status = 401;
        return { success: false, message: t("server.passkeyRoutes.bindTokenExpired") };
      }
      const { generateRegistrationOptions } = await loadSimpleWebAuthnServer();
      const { rpID } = await getRpInfo(request);
      const passkeys = await configManager.getPasskeys();
      const options = await generateRegistrationOptions({
        rpName: RP_NAME,
        rpID,
        userID: new TextEncoder().encode("admin"),
        userName: "admin",
        userDisplayName: "admin",
        attestationType: "none",
        excludeCredentials: passkeys.map((passkey) => ({
          id: passkey.id,
          type: "public-key" as const,
          transports: passkey.transports as any,
        })),
      });
      await configManager.setPasskeyChallenge(options.challenge, "register");
      return { success: true, data: options };
    },
    withRouteDoc("创建 Passkey 注册选项", {
      body: t.Object({
        token: t.String(),
      }),
    }),
  )
  .post(
    "/register/verify",
    async ({ body, set, request }) => {
      const { t } = await getPasskeyRouteTranslator(request);
      const totpId = await configManager.consumePasskeyBindToken(body.token);
      if (!totpId) {
        set.status = 401;
        return { success: false, message: t("server.passkeyRoutes.bindTokenExpired") };
      }
      const credential = body.credential;
      const challenge = extractChallenge(credential?.response?.clientDataJSON);
      if (!challenge) {
        set.status = 400;
        return { success: false, message: "Invalid passkey response" };
      }
      const isChallengeValid = await configManager.consumePasskeyChallenge(
        challenge,
        "register",
      );
      if (!isChallengeValid) {
        set.status = 400;
        return { success: false, message: "Passkey challenge expired" };
      }
      const { origin, rpID } = await getRpInfo(request);
      let verification;
      try {
        const { verifyRegistrationResponse } = await loadSimpleWebAuthnServer();
        verification = await verifyRegistrationResponse({
          response: credential,
          expectedChallenge: challenge,
          expectedOrigin: origin,
          expectedRPID: rpID,
          requireUserVerification: false,
        });
      } catch {
        set.status = 400;
        return { success: false, message: "Passkey registration failed" };
      }
      if (!verification.verified || !verification.registrationInfo) {
        set.status = 400;
        return { success: false, message: "Passkey registration failed" };
      }
      const info = verification.registrationInfo;
      const passkeys = await configManager.getPasskeys();
      if (passkeys.some((passkey) => passkey.id === info.credential.id)) {
        set.status = 409;
        return { success: false, message: "Passkey already registered" };
      }
      const passkey: PasskeyCredential = {
        id: info.credential.id,
        totpId,
        publicKey: bufferToBase64Url(Buffer.from(info.credential.publicKey)),
        counter: 0,
        transports: credential.transports,
        deviceName: body.deviceName || "Unknown Device",
        createdAt: new Date().toISOString(),
      };
      await configManager.addPasskey(passkey);
      return { success: true };
    },
    withRouteDoc("校验并保存 Passkey 注册", {
      body: t.Object({
        token: t.String(),
        deviceName: t.Optional(t.String()),
        credential: t.Any(),
      }),
    }),
  );
