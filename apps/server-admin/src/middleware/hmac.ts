import { Elysia } from "elysia";
import { createHmac } from "node:crypto";
import { configManager } from "../lib/redis";
import { getRequiredEnv } from "../lib/env";
import { safeEqualString } from "../lib/security";

const HMAC_SECRET = getRequiredEnv("HMAC_SECRET");

const IGNORED_PATHS = new Set([
    "/api/auth/challenge",
    "/api/auth/verify",
    "/api/auth/logout",
    "/api/auth/preflight",
]);

export const hmacMiddleware = new Elysia({ name: 'hmac-middleware' })
    .onBeforeHandle({ as: 'global' }, async ({ request, path, set }) => {
        const normalizedPath = path.startsWith("/auth/api") ? path.slice("/auth".length) : path;
        if (!normalizedPath.startsWith("/api") || IGNORED_PATHS.has(normalizedPath)) {
            return;
        }
        const timestampStr = request.headers.get("x-timestamp");
        const nonce = request.headers.get("x-nonce");
        const signature = request.headers.get("x-signature");

        if (!timestampStr || !nonce || !signature) {
            set.status = 401;
            return {
                success: false,
                message: "Missing Required Security Headers (x-timestamp, x-nonce, x-signature)"
            };
        }

        const timestamp = parseInt(timestampStr, 10);
        if (isNaN(timestamp)) {
            set.status = 400;
            return { success: false, message: "Invalid Timestamp Format" };
        }

        const now = Date.now();
        const timeDiff = Math.abs(now - timestamp);
        if (timeDiff > 5 * 60 * 1000) {
            set.status = 401;
            return { success: false, message: "Request Expired or Time Desynced" };
        }

        if (nonce.length < 8) {
            set.status = 400;
            return { success: false, message: "Invalid Nonce Length" };
        }
        
        const expectedMessage = `${timestamp}:${nonce}`;
        const hmac = createHmac("sha256", HMAC_SECRET);
        hmac.update(expectedMessage);
        const expectedSignature = hmac.digest("hex");

        if (!safeEqualString(signature.toLowerCase(), expectedSignature)) {
            set.status = 401;
            return { success: false, message: "Invalid Signature" };
        }

        const isNewNonce = await configManager.setNonceIfNotExists(nonce, 600);
        if (!isNewNonce) {
            set.status = 401;
            return { success: false, message: "Replay Attack Detected: Nonce already used" };
        }
    });
