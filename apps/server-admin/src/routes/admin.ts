import { Elysia, t } from "elysia";
import { configManager, type ProxyMapping, type RunModePromptPreferences, type SSLConfig, type SSLStatus } from "../lib/redis";
import { generateSecret, generateURI, verifySync } from "otplib";
import { goBackend } from "../lib/go-backend";
import { firewallService } from "../lib/firewall-service";
import { randomBytes } from "node:crypto";
import { authLogManager } from "../lib/auth-log";
import { authMobilitySessionManager } from "../lib/auth-mobility-session";
import { scanDetector } from "../lib/scan-detector";

const parseIntSafe = (value: string | undefined, fallback: number) => {
    const v = Number.parseInt(String(value ?? ""), 10);
    if (!Number.isFinite(v)) return fallback;
    return v;
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const buildCountSeries = (timestamps: number[], fromMs: number, toMs: number, bucketCount: number) => {
    const span = Math.max(1, toMs - fromMs);
    const step = Math.max(1, Math.ceil(span / bucketCount));
    const buckets = Array.from({ length: bucketCount }, () => 0);
    for (const ts of timestamps) {
        if (!Number.isFinite(ts)) continue;
        const idx = Math.min(bucketCount - 1, Math.max(0, Math.floor((ts - fromMs) / step)));
        const current = buckets[idx] ?? 0;
        buckets[idx] = current + 1;
    }
    return buckets.map((count, index) => [fromMs + index * step, count] as [number, number]);
};

export const adminRoutes = new Elysia({ prefix: "/api/admin" })
    .get("/onboarding/status", async () => {
        const status = await configManager.getOnboardingStatus();
        return { success: true, data: status };
    })
    .post("/onboarding/complete", async () => {
        await configManager.markOnboardingCompleted();
        return { success: true };
    })
    .get("/config", async () => {
        const config = await configManager.getConfigSafe();
        return { success: true, data: config };
    })
    .post("/config/run_type", async ({ body }) => {
        const config = await configManager.getConfig();
        const previousRunType = config.run_type;
        await configManager.updateRunType(body.run_type);
        await firewallService.applyRunTypeConfig(body.run_type, previousRunType);
        return { success: true };
    }, {
        body: t.Object({
            run_type: t.Union([t.Literal(0), t.Literal(1)])
        })
    })
    .get("/config/run_mode_prompt_preferences", async () => {
        const preferences = await configManager.getRunModePromptPreferences();
        return { success: true, data: preferences };
    })
    .post("/config/run_mode_prompt_preferences", async ({ body }) => {
        const patch: Partial<RunModePromptPreferences> = {};

        if (body.directToReverseProxy !== undefined) {
            patch.directToReverseProxy = body.directToReverseProxy;
        }
        if (body.reverseProxyToDirect !== undefined) {
            patch.reverseProxyToDirect = body.reverseProxyToDirect;
        }

        const preferences = await configManager.updateRunModePromptPreferences(patch);
        return { success: true, data: preferences };
    }, {
        body: t.Object({
            directToReverseProxy: t.Optional(t.Boolean()),
            reverseProxyToDirect: t.Optional(t.Boolean()),
        })
    })
    .get("/config/fnos_share_bypass", async () => {
        const settings = await configManager.getFnosShareBypassConfig();
        return { success: true, data: settings };
    })
    .post("/config/fnos_share_bypass", async ({ body }) => {
        const next = await configManager.updateFnosShareBypassConfig(body);
        return { success: true, data: next };
    }, {
        body: t.Object({
            enabled: t.Optional(t.Boolean()),
            upstream_timeout_ms: t.Optional(t.Number()),
            validation_cache_ttl_seconds: t.Optional(t.Number()),
            validation_lock_ttl_seconds: t.Optional(t.Number()),
            session_ttl_seconds: t.Optional(t.Number()),
        })
    })
    .get("/config/captcha", async () => {
        const settings = await configManager.getCaptchaSettings();
        return { success: true, data: settings };
    })
    .post("/config/captcha", async ({ body, set }) => {
        if (body.provider === 'turnstile') {
            const siteKey = body.turnstile?.site_key?.trim() || '';
            const secretKey = body.turnstile?.secret_key?.trim() || '';
            if (!siteKey || !secretKey) {
                set.status = 400;
                return {
                    success: false,
                    message: '启用 Cloudflare Turnstile 时，site_key 和 secret_key 都必须填写',
                };
            }
        }

        const next = await configManager.updateCaptchaSettings({
            provider: body.provider,
            turnstile: body.turnstile,
        });
        return { success: true, data: next };
    }, {
        body: t.Object({
            provider: t.Union([t.Literal('pow'), t.Literal('turnstile')]),
            turnstile: t.Object({
                site_key: t.String(),
                secret_key: t.String(),
            }),
        })
    })
    .get("/config/default_route", async () => {
        const config = await configManager.getConfig();
        return { success: true, data: { default_route: config.default_route } };
    })
    .post("/config/default_route", async ({ body }) => {
        await configManager.updateDefaultRoute(body.path);
        await goBackend.setDefaultRoute(body.path);
        return { success: true };
    }, {
        body: t.Object({
            path: t.String()
        })
    })
    .post("/config/default_tunnel", async ({ body }) => {
        await configManager.updateDefaultTunnel(body.tunnel);
        return { success: true };
    }, {
        body: t.Object({
            tunnel: t.Union([t.Literal('frp'), t.Literal('cloudflared')])
        })
    })
    .post("/config/proxy_mappings", async ({ body }) => {
        await configManager.updateProxyMappings(body.mappings);
        await goBackend.setRules(body.mappings);
        return { success: true };
    }, {
        body: t.Object({
            mappings: t.Array(t.Object({
                path: t.String(),
                target: t.String(),
                rewrite_html: t.Boolean(),
                use_auth: t.Boolean(),
                use_root_mode: t.Boolean(),
                strip_path: t.Boolean()
            }))
        })
    })
    // TOTP 认证管理
    .get("/totp/status", async () => {
        const credentials = await configManager.getTOTPCredentials();
        return { success: true, data: { bound: credentials.length > 0, credentials } };
    })
    .post("/totp/setup", async () => {
        const secret = generateSecret();
        const uri = generateURI({ issuer: 'fn-knock', label: 'admin', secret, strategy: 'totp' });
        return { success: true, data: { secret, uri } };
    })
    .post("/totp/bind", async ({ body, set }) => {
        const { valid } = verifySync({ strategy: 'totp', token: body.token, secret: body.secret });
        if (!valid) {
            set.status = 400;
            return { success: false, message: '验证码不正确，请重试' };
        }
        await configManager.addTOTPCredential({
            id: randomBytes(8).toString('hex'),
            secret: body.secret,
            comment: body.comment || 'New Token',
            createdAt: new Date().toISOString()
        });
        return { success: true };
    }, {
        body: t.Object({
            secret: t.String(),
            token: t.String(),
            comment: t.Optional(t.String())
        })
    })
    .delete("/totp/:id", async ({ params, set }) => {
        const deleted = await configManager.deleteTOTPCredential(params.id);
        if (!deleted) {
            set.status = 404;
            return { success: false, message: "TOTP not found" };
        }
        return { success: true };
    }, {
        params: t.Object({ id: t.String() })
    })
    .patch("/totp/:id/comment", async ({ params, body, set }) => {
        const updated = await configManager.updateTOTPCredential(params.id, body.comment);
        if (!updated) {
            set.status = 404;
            return { success: false, message: "TOTP not found" };
        }
        return { success: true };
    }, {
        params: t.Object({ id: t.String() }),
        body: t.Object({ comment: t.String() })
    })
    .get("/totp/:totpId/passkeys", async ({ params }) => {
        const passkeys = await configManager.getPasskeys();
        const filtered = passkeys.filter(pk => pk.totpId === params.totpId);
        return { success: true, data: filtered };
    }, {
        params: t.Object({ totpId: t.String() })
    })
    .delete("/passkeys/:id", async ({ params, set }) => {
        const deleted = await configManager.deletePasskey(params.id);
        if (!deleted) {
            set.status = 404;
            return { success: false, message: "Passkey not found" };
        }
        return { success: true };
    }, {
        params: t.Object({
            id: t.String()
        })
    })
    .post("/sync-routes", async ({ set }) => {
        try {
            const config = await configManager.getConfig();
            // 同步代理规则
            const rulesResult = await goBackend.setRules(config.proxy_mappings);
            // 同步默认路由
            const routeResult = await goBackend.setDefaultRoute(config.default_route);
            if (!rulesResult.success || !routeResult.success) {
                set.status = 502;
                return {
                    success: false,
                    message: `同步部分失败: rules=${rulesResult.success}, default_route=${routeResult.success}`,
                };
            }
            return {
                success: true,
                data: { synced_rules: config.proxy_mappings.length },
                message: `成功同步 ${config.proxy_mappings.length} 条路由规则`,
            };
        } catch (e: any) {
            set.status = 500;
            return { success: false, message: e?.message ?? String(e) };
        }
    })
    .get("/logs", async ({ query }) => {
        const page = parseInt(query.page || "1", 10);
        const limit = parseInt(query.limit || "50", 10);
        const search = query.search || "";
        const result = await authLogManager.getLogs(page, limit, search);
        return { success: true, data: result };
    }, {
        query: t.Object({
            page: t.Optional(t.String()),
            limit: t.Optional(t.String()),
            search: t.Optional(t.String())
        })
    })
    .delete("/logs", async ({ body }) => {
        await authLogManager.deleteLogs(body.ids);
        return { success: true };
    }, {
        body: t.Object({
            ids: t.Array(t.String())
        })
    })
    .get("/security/overview", async ({ query }) => {
        const rangeSec = clamp(parseIntSafe(query.rangeSec, 3600), 60, 30 * 24 * 3600);
        const nowMs = Date.now();
        const fromMs = nowMs - rangeSec * 1000;
        const bucketCount = Math.min(48, Math.max(12, Math.round(rangeSec / 900)));
        const logs = await authLogManager.listLogsByRange(fromMs, nowMs);
        const failedTimestamps = logs
            .filter((item) => item.log.type === "login" && !item.log.success)
            .map((item) => item.timestamp);
        const blockedRecords = await scanDetector.listBlacklistByRange(fromMs, nowMs);
        const blockedTimestamps = blockedRecords.map((item) => item.blockedAt);

        return {
            success: true,
            data: {
                rangeSec,
                totals: {
                    failedLogins: failedTimestamps.length,
                    blockedScanners: blockedTimestamps.length
                },
                series: {
                    failedLogins: buildCountSeries(failedTimestamps, fromMs, nowMs, bucketCount),
                    blockedScanners: buildCountSeries(blockedTimestamps, fromMs, nowMs, bucketCount)
                }
            }
        };
    }, {
        query: t.Object({
            rangeSec: t.Optional(t.String())
        })
    })
    // Session management
    .get("/sessions", async () => {
        const list = await configManager.listSessions();
        const mapped = await Promise.all(
            list.map(async ({ id, data }) => ({
                id,
                ...data,
                mobility: await authMobilitySessionManager.getSessionMobilitySummary(id),
            }))
        );
        return { success: true, data: mapped };
    })
    .get("/sessions/:id", async ({ params, set }) => {
        const sess = await configManager.getSession(params.id);
        if (!sess) {
            set.status = 404;
            return { success: false, message: "Session not found" };
        }
        return { success: true, data: { id: params.id, ...sess } };
    }, {
        params: t.Object({ id: t.String() })
    })
    .get("/sessions/:id/mobility", async ({ params, set }) => {
        const sess = await configManager.getSession(params.id);
        if (!sess) {
            set.status = 404;
            return { success: false, message: "Session not found" };
        }
        return {
            success: true,
            data: await authMobilitySessionManager.getSessionMobilityDetails(params.id),
        };
    }, {
        params: t.Object({ id: t.String() })
    })
    .delete("/sessions/:id", async ({ params }) => {
        const sess = await configManager.getSession(params.id);
        if (sess) {
            await authMobilitySessionManager.destroySession(params.id);
            await configManager.deleteSession(params.id);
            await authLogManager.recordLog({
                type: "logout",
                ip: sess.ip,
                userAgent: sess.userAgent,
                success: true
            });
        } else {
            await configManager.deleteSession(params.id);
        }
        return { success: true };
    }, {
        params: t.Object({ id: t.String() })
    });
