import { Elysia, t } from "elysia";
import { configManager, type PasskeyCredential } from "../../lib/redis";
import {
  base64UrlToBuffer,
  bufferToBase64Url,
  extractChallenge,
  getRpInfo,
  handleLoginSuccess,
} from "../../lib/auth-utils";
import { getClientIp } from "../../lib/auth-request";
import { authLogManager } from "../../lib/auth-log";
import {
  generateAuthenticationOptions,
  generateRegistrationOptions,
  verifyAuthenticationResponse,
  verifyRegistrationResponse,
} from "@simplewebauthn/server";

const RP_NAME = "fn-knock";

export const passkeyRoutes = new Elysia({ prefix: "/passkey" })
  .get("/status", async () => {
    const passkeys = await configManager.getPasskeys();
    return { success: true, data: { available: passkeys.length > 0 } };
  })
  .post("/auth/options", async ({ set, request }) => {
    const passkeys = await configManager.getPasskeys();
    if (passkeys.length === 0) {
      set.status = 404;
      return { success: false, message: "No passkey available" };
    }
    const { rpID } = getRpInfo(request);
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
  })
  .post(
    "/auth/verify",
    async ({ body, set, request }) => {
      const { origin, rpID } = getRpInfo(request);
      const clientIp = getClientIp(request);
      const userAgent = request.headers.get("user-agent") || "Unknown";
      
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
        await authLogManager.recordLog({
          type: "login",
          method: "PASSKEY",
          ip: clientIp,
          userAgent,
          success: false,
          credentialName: "Unknown Passkey",
        });
        set.status = 404;
        return { success: false, message: "Passkey not found" };
      }

      let verification;
      try {
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
        await authLogManager.recordLog({
          type: "login",
          method: "PASSKEY",
          ip: clientIp,
          userAgent,
          success: false,
          credentialName: matched.deviceName,
        });
        set.status = 400;
        return { success: false, message: `验证失败: ${error.message}` };
      }
      await configManager.updatePasskeyCounter(
        matched.id,
        verification.authenticationInfo.newCounter,
        new Date().toISOString(),
      );
      const config = await configManager.getConfig();
      return await handleLoginSuccess({
        config,
        clientIp,
        userAgent,
        authMethod: "PASSKEY",
        credentialId: matched.id,
        credentialName: matched.deviceName,
        rememberMe: body.rememberMe,
        set,
        totpId: matched.totpId,
      });
    },
    {
      body: t.Object({
        credential: t.Any(),
        rememberMe: t.Boolean(),
      }),
    },
  )
  .post("/bind-token", async ({ set, request }) => {
    const cookieHeader = request.headers.get("cookie") || "";
    const match = cookieHeader.match(/x-go-reauth-proxy-session-id=([^;]+)/);
    let totpId = "";
    if (match && match[1]) {
      const session = await configManager.getSession(match[1]);
      totpId = session?.totpId || "";
    }
    if (!totpId) {
      set.status = 401;
      return { success: false, message: "Unauthorized or missing TOTP ID" };
    }

    const passkeys = await configManager.getPasskeys();
    const boundPasskeys = passkeys.filter(pk => pk.totpId === totpId);
    if (boundPasskeys.length > 0) {
      set.status = 409;
      return { success: false, message: "Passkey already bound" };
    }
    const token = await configManager.createPasskeyBindToken(totpId);
    return { success: true, data: { token } };
  })
  .post(
    "/register/options",
    async ({ body, set, request }) => {
      const isTokenValid = await configManager.isPasskeyBindTokenValid(
        body.token,
      );
      if (!isTokenValid) {
        set.status = 401;
        return { success: false, message: "绑定凭证已失效" };
      }
      const { rpID } = getRpInfo(request);
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
    {
      body: t.Object({
        token: t.String(),
      }),
    },
  )
  .post(
    "/register/verify",
    async ({ body, set, request }) => {
      const totpId = await configManager.consumePasskeyBindToken(
        body.token,
      );
      if (!totpId) {
        set.status = 401;
        return { success: false, message: "绑定凭证已失效" };
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
      const { origin, rpID } = getRpInfo(request);
      let verification;
      try {
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
    {
      body: t.Object({
        token: t.String(),
        deviceName: t.Optional(t.String()),
        credential: t.Any(),
      }),
    },
  );
