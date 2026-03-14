import { createHash, createHmac, randomBytes } from "node:crypto";
import {
    configManager,
    type CaptchaProvider,
    type CaptchaSettings,
} from "./redis";
import { safeEqualString } from "./security";

const MAX_NUMBER = 100000;
const TURNSTILE_VERIFY_URL = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

type PowChallengeResponse = {
    algorithm: "SHA-256";
    challenge: string;
    maxnumber: number;
    salt: string;
    signature: string;
};

type CaptchaSubmission =
    | {
        provider: "pow";
        proof: string;
    }
    | {
        provider: "turnstile";
        token: string;
    };

type TurnstileVerifyResponse = {
    success?: boolean;
    "error-codes"?: string[];
};

export type PublicCaptchaSettings = {
    provider: CaptchaProvider;
    widget_mode: "normal";
    available: boolean;
    unavailable_reason: string | null;
    pow: Record<string, never>;
    turnstile: {
        site_key: string;
    };
};

interface CaptchaVerifyContext {
    clientIp: string;
}

interface CaptchaProviderAdapter {
    readonly provider: CaptchaProvider;
    verify(
        submission: CaptchaSubmission,
        settings: CaptchaSettings,
        context: CaptchaVerifyContext,
    ): Promise<void>;
    createChallenge?(settings: CaptchaSettings): Promise<PowChallengeResponse>;
}

class PowCaptchaProvider implements CaptchaProviderAdapter {
    readonly provider = "pow" as const;

    private getHmacKey() {
        const key = process.env.ALTCHA_HMAC_KEY?.trim();
        if (!key) {
            throw new Error("PoW 验证码尚未完成服务端配置");
        }
        return key;
    }

    isAvailable(): { available: boolean; reason: string | null } {
        const key = process.env.ALTCHA_HMAC_KEY?.trim();
        if (!key) {
            return {
                available: false,
                reason: "PoW 验证码尚未完成服务端配置",
            };
        }
        return { available: true, reason: null };
    }

    async createChallenge(_settings: CaptchaSettings): Promise<PowChallengeResponse> {
        const hmacKey = this.getHmacKey();
        const salt = randomBytes(12).toString("hex");
        const expires = Math.floor(Date.now() / 1000) + 300;
        const saltWithParams = `${salt}?expires=${expires}`;
        const secretNumber = Math.floor(Math.random() * MAX_NUMBER);
        const challenge = createHash("sha256")
            .update(saltWithParams + secretNumber.toString())
            .digest("hex");
        const signature = createHmac("sha256", hmacKey)
            .update(challenge)
            .digest("hex");

        return {
            algorithm: "SHA-256",
            challenge,
            maxnumber: MAX_NUMBER,
            salt: saltWithParams,
            signature,
        };
    }

    async verify(
        submission: CaptchaSubmission,
        _settings: CaptchaSettings,
    ): Promise<void> {
        if (submission.provider !== "pow") {
            throw new Error("验证码类型不匹配");
        }

        const payloadDecoded = Buffer.from(submission.proof, "base64").toString("utf-8");
        const data = JSON.parse(payloadDecoded) as {
            algorithm?: string;
            challenge?: string;
            number?: number;
            salt?: string;
            signature?: string;
        };

        if (data.algorithm !== "SHA-256") {
            throw new Error("Invalid algorithm");
        }

        const numberText = typeof data.number === "number" ? data.number.toString() : "";
        const expectedChallenge = createHash("sha256")
            .update(String(data.salt || "") + numberText)
            .digest("hex");
        if (!safeEqualString(String(data.challenge || "").toLowerCase(), expectedChallenge)) {
            throw new Error("Invalid challenge");
        }

        const expectedSignature = createHmac("sha256", this.getHmacKey())
            .update(String(data.challenge || ""))
            .digest("hex");
        if (!safeEqualString(String(data.signature || "").toLowerCase(), expectedSignature)) {
            throw new Error("Invalid signature");
        }

        const expiresMatch = String(data.salt || "").match(/expires=(\d+)/);
        if (expiresMatch) {
            const expires = parseInt(expiresMatch[1], 10);
            if (Date.now() / 1000 > expires) {
                throw new Error("Challenge expired");
            }
        }

        const isNewChallenge = await configManager.setNonceIfNotExists(
            String(data.challenge || ""),
            86400,
        );
        if (!isNewChallenge) {
            throw new Error("Challenge has already been used");
        }
    }
}

class TurnstileCaptchaProvider implements CaptchaProviderAdapter {
    readonly provider = "turnstile" as const;

    isAvailable(settings: CaptchaSettings): { available: boolean; reason: string | null } {
        if (!settings.turnstile.site_key.trim() || !settings.turnstile.secret_key.trim()) {
            return {
                available: false,
                reason: "当前 Turnstile 未完成配置，请联系管理员完善参数",
            };
        }
        return { available: true, reason: null };
    }

    async verify(
        submission: CaptchaSubmission,
        settings: CaptchaSettings,
        context: CaptchaVerifyContext,
    ): Promise<void> {
        if (submission.provider !== "turnstile") {
            throw new Error("验证码类型不匹配");
        }

        const secretKey = settings.turnstile.secret_key.trim();
        if (!secretKey) {
            throw new Error("Cloudflare Turnstile secret_key 未配置");
        }

        const token = submission.token.trim();
        if (!token) {
            throw new Error("Turnstile token 不能为空");
        }

        const body = new URLSearchParams({
            secret: secretKey,
            response: token,
        });
        if (context.clientIp) {
            body.set("remoteip", context.clientIp);
        }

        const response = await fetch(TURNSTILE_VERIFY_URL, {
            method: "POST",
            headers: {
                "Content-Type": "application/x-www-form-urlencoded",
            },
            body,
        });

        if (!response.ok) {
            throw new Error("Turnstile 校验服务暂时不可用");
        }

        const result = await response.json() as TurnstileVerifyResponse;
        if (result.success !== true) {
            const reason = result["error-codes"]?.filter(Boolean).join(", ");
            throw new Error(reason ? `Turnstile 验证失败: ${reason}` : "Turnstile 验证失败");
        }
    }
}

class CaptchaService {
    private readonly providers = new Map<CaptchaProvider, CaptchaProviderAdapter>([
        ["pow", new PowCaptchaProvider()],
        ["turnstile", new TurnstileCaptchaProvider()],
    ]);

    async getSettings(): Promise<CaptchaSettings> {
        return configManager.getCaptchaSettings();
    }

    async getPublicSettings(): Promise<PublicCaptchaSettings> {
        const settings = await this.getSettings();
        const status = this.getAvailability(settings);
        return {
            provider: settings.provider,
            widget_mode: "normal",
            available: status.available,
            unavailable_reason: status.reason,
            pow: {},
            turnstile: {
                site_key: settings.turnstile.site_key,
            },
        };
    }

    private getAvailability(settings: CaptchaSettings): {
        available: boolean;
        reason: string | null;
    } {
        if (settings.provider === "pow") {
            return new PowCaptchaProvider().isAvailable();
        }

        if (settings.provider === "turnstile") {
            return new TurnstileCaptchaProvider().isAvailable(settings);
        }

        return {
            available: false,
            reason: "未找到可用的验证码提供商",
        };
    }

    async createChallenge(): Promise<PowChallengeResponse> {
        const settings = await this.getSettings();
        if (settings.provider !== "pow") {
            throw new Error("当前未启用 PoW 验证码");
        }

        const availability = this.getAvailability(settings);
        if (!availability.available) {
            throw new Error(availability.reason || "当前 PoW 验证码不可用");
        }

        const provider = this.providers.get("pow");
        if (!provider?.createChallenge) {
            throw new Error("PoW provider is unavailable");
        }
        return provider.createChallenge(settings);
    }

    async verify(
        submission: CaptchaSubmission,
        context: CaptchaVerifyContext,
    ): Promise<void> {
        const settings = await this.getSettings();
        if (submission.provider !== settings.provider) {
            throw new Error("验证码提供商与当前配置不一致");
        }

        const provider = this.providers.get(settings.provider);
        if (!provider) {
            throw new Error("未找到可用的验证码提供商");
        }

        await provider.verify(submission, settings, context);
    }
}

export const captchaService = new CaptchaService();
