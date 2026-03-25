import { Elysia, t } from "elysia";
import {
  type AppConfig,
  configManager,
  DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  type HostMapping,
  type LoginSession,
  type ProtocolMappingFeatureConfig,
  type ProxyMapping,
  type RunModePromptPreferences,
  type StreamMapping,
} from "../lib/redis";
import { generateSecret, generateURI, verifySync } from "otplib";
import { goBackend } from "../lib/go-backend";
import { firewallService } from "../lib/firewall-service";
import { randomBytes } from "node:crypto";
import { authLogManager } from "../lib/auth-log";
import { authMobilitySessionManager } from "../lib/auth-mobility-session";
import { ipLocationRefs, ipLocationService } from "../lib/ip-location";
import { scanDetector } from "../lib/scan-detector";
import { whitelistManager } from "../lib/whitelist-manager";
import {
  buildGatewayAuthConfig,
  buildSubdomainCertificateInventoryCoverage,
  getAuthHostMapping,
} from "../lib/subdomain-mode";
import { isAuthServiceTarget } from "../lib/auth-service";
import {
  enrichHostMappingsMetadataOnSave,
  refreshAllHostMappingTitles,
} from "../lib/host-mapping-metadata";
import {
  buildHostMappingsBookmarkFilename,
  buildHostMappingsBookmarksDocument,
} from "../lib/host-mapping-bookmarks";
import { getGatewayLoggingConfigForResponse } from "../lib/gateway-logging";
import { syncSSLDeploymentToGateway } from "../lib/ssl-gateway";
import {
  MaintenanceBackupError,
  maintenanceBackupService,
} from "../lib/maintenance-backup";

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

const getRunTypeLabel = (runType: 0 | 1 | 3) => {
  if (runType === 0) return "直连模式";
  if (runType === 1) return "反代模式";
  return "子域模式";
};

const validateHostMappings = (
  mappings: Array<{
    host: string;
    target: string;
    use_auth: boolean;
    access_mode: "login_first" | "strict_whitelist";
    suppress_toolbar?: boolean;
    service_role?: "app" | "auth";
  }>,
) => {
  const authMappings = mappings.filter((mapping) =>
    isAuthServiceTarget(mapping.target),
  );
  if (authMappings.length > 1) {
    return {
      valid: false as const,
      message: "只能有一个 Host 映射指向 AUTH_PORT 作为鉴权服务",
    };
  }

  const invalidAuthMapping = authMappings.find(
    (mapping) => mapping.use_auth || mapping.access_mode === "strict_whitelist",
  );
  if (invalidAuthMapping) {
    return {
      valid: false as const,
      message: `鉴权服务 ${invalidAuthMapping.host} 必须保持公开入口，不能开启自身鉴权或严格白名单，否则会导致登录入口不可达`,
    };
  }

  return { valid: true as const };
};

const isValidStreamTarget = (target: string): boolean => {
  const trimmed = target.trim();
  if (!trimmed) return false;

  try {
    const parsed = new URL(`tcp://${trimmed}`);
    const port = Number.parseInt(parsed.port, 10);
    if (!parsed.hostname) return false;
    if (!Number.isFinite(port) || port <= 0 || port > 65535) return false;
    if (parsed.pathname && parsed.pathname !== "/") return false;
    if (parsed.search || parsed.hash) return false;
    return true;
  } catch {
    return false;
  }
};

type StreamMappingInput = Pick<
  StreamMapping,
  "listen_port" | "target" | "use_auth"
> & {
  protocol?: StreamMapping["protocol"];
};

const validateStreamMappings = (mappings: StreamMappingInput[]) => {
  const seenMappings = new Set<string>();

  for (const mapping of mappings) {
    const protocol = mapping.protocol === "udp" ? "udp" : "tcp";
    const listenPort = mapping.listen_port;
    const target = mapping.target;

    if (!Number.isInteger(listenPort)) {
      return {
        valid: false as const,
        message: `监听端口 ${listenPort} 不是有效整数`,
      };
    }
    if (listenPort <= 0 || listenPort > 65535) {
      return {
        valid: false as const,
        message: `监听端口 ${listenPort} 超出有效范围`,
      };
    }
    const mappingKey = `${protocol}:${listenPort}`;
    if (seenMappings.has(mappingKey)) {
      return {
        valid: false as const,
        message: `${protocol.toUpperCase()} 监听端口 ${listenPort} 重复，请保持协议 + 端口唯一`,
      };
    }
    if (!isValidStreamTarget(target)) {
      return {
        valid: false as const,
        message: `目标地址 ${target} 必须是 host:port 形式`,
      };
    }
    seenMappings.add(mappingKey);
  }

  return { valid: true as const };
};

const normalizeHostLike = (value: string | undefined | null): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const resolveRequestScheme = (request: Request): "http" | "https" => {
  const forwarded = request.headers
    .get("x-forwarded-proto")
    ?.split(",")[0]
    ?.trim()
    .toLowerCase();
  if (forwarded === "http" || forwarded === "https") {
    return forwarded;
  }

  try {
    return new URL(request.url).protocol === "http:" ? "http" : "https";
  } catch {
    return "https";
  }
};

const validatePasskeyRpConfig = (
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
) => {
  const mode =
    config.subdomain_mode?.passkey_rp_mode === "parent_domain"
      ? "parent_domain"
      : "auth_host";
  if (mode !== "parent_domain") {
    return { valid: true as const };
  }

  const rpId = normalizeHostLike(
    config.subdomain_mode?.passkey_rp_id || config.subdomain_mode?.root_domain,
  );
  if (!rpId) {
    return {
      valid: false as const,
      message:
        "启用父域 Passkey RP 时，请先填写根域名，或显式指定一个父域 RP ID。",
    };
  }

  const authHost = normalizeHostLike(
    getAuthHostMapping(config)?.host || config.subdomain_mode?.auth_host,
  );
  if (authHost && authHost !== rpId && !authHost.endsWith(`.${rpId}`)) {
    return {
      valid: false as const,
      message: `父域 Passkey RP ID ${rpId} 必须与鉴权服务 ${authHost} 相同，或是它的父域。`,
    };
  }

  return { valid: true as const };
};

const resolveSessionDefaultComment = async (
  sessionId: string,
  session: LoginSession,
): Promise<string | undefined> => {
  const whitelistRecordId =
    await authMobilitySessionManager.getSessionWhitelistRecordId(sessionId);
  const boundRecord = whitelistRecordId
    ? await whitelistManager.getRecordById(whitelistRecordId)
    : null;
  if (boundRecord?.status === "active" && boundRecord.comment !== undefined) {
    return boundRecord.comment;
  }

  const latestRecord = await whitelistManager.getLatestActiveRecordByIP(
    session.ip,
  );
  if (!latestRecord || latestRecord.comment === undefined) {
    return undefined;
  }

  return latestRecord.comment;
};

const ensureSessionComment = async (
  sessionId: string,
  session: LoginSession,
): Promise<LoginSession> => {
  if (session.comment !== undefined) {
    return session;
  }

  const comment = await resolveSessionDefaultComment(sessionId, session);
  if (comment === undefined) {
    return session;
  }

  return (
    (await configManager.updateSession(sessionId, { comment })) ?? {
      ...session,
      comment,
    }
  );
};

const rollbackConfigAndRuntime = async (
  previousConfig: AppConfig,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复之前的配置失败";
  }

  try {
    await firewallService.applyRunTypeConfig(
      previousConfig.run_type,
      previousConfig.run_type,
    );
  } catch (error: any) {
    return error?.message || "恢复之前的运行态失败";
  }

  return null;
};

const rollbackProtocolMappingFeatureAndRuntime = async (
  previousSettings: ProtocolMappingFeatureConfig,
  previousConfig: AppConfig,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复协议映射配置失败";
  }

  try {
    await configManager.updateProtocolMappingFeatureConfig(previousSettings);
  } catch (error: any) {
    return error?.message || "恢复协议映射功能开关失败";
  }

  try {
    await firewallService.applyRunTypeConfig(
      previousConfig.run_type,
      previousConfig.run_type,
    );
  } catch (error: any) {
    return error?.message || "恢复协议映射运行态失败";
  }

  return null;
};

const buildGatewaySettingsResponse = (
  config: Pick<AppConfig, "subdomain_mode" | "reverse_proxy_throttle">,
) => ({
  auth_cache_ttl_seconds: config.subdomain_mode?.auth_cache_ttl_seconds ?? 1,
  auth_cache_unauthorized_ttl_seconds:
    config.subdomain_mode?.auth_cache_unauthorized_ttl_seconds ?? 1,
  reverse_proxy_throttle: config.reverse_proxy_throttle ?? {
    ...DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  },
});

const buildCountSeries = (
  timestamps: number[],
  fromMs: number,
  toMs: number,
  bucketCount: number,
) => {
  const span = Math.max(1, toMs - fromMs);
  const step = Math.max(1, Math.ceil(span / bucketCount));
  const buckets = Array.from({ length: bucketCount }, () => 0);
  for (const ts of timestamps) {
    if (!Number.isFinite(ts)) continue;
    const idx = Math.min(
      bucketCount - 1,
      Math.max(0, Math.floor((ts - fromMs) / step)),
    );
    const current = buckets[idx] ?? 0;
    buckets[idx] = current + 1;
  }
  return buckets.map(
    (count, index) => [fromMs + index * step, count] as [number, number],
  );
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
    const [config, gatewayLogging] = await Promise.all([
      configManager.getConfigSafe(),
      getGatewayLoggingConfigForResponse(),
    ]);

    return {
      success: true,
      data: {
        ...config,
        gateway_logging: gatewayLogging,
      },
    };
  })
  .post(
    "/config/run_type",
    async ({ body, set }) => {
      const [config, previousProtocolMappingFeature] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      const previousRunType = config.run_type;
      try {
        await configManager.updateRunType(body.run_type);
        if (body.run_type !== 3) {
          await configManager.updateProtocolMappingFeatureConfig({
            enabled: false,
          });
        }
        await firewallService.applyRunTypeConfig(
          body.run_type,
          previousRunType,
        );
      } catch (error: any) {
        const rollbackError = await rollbackProtocolMappingFeatureAndRuntime(
          previousProtocolMappingFeature,
          config,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "切换运行模式失败"}；回滚失败：${rollbackError}`
            : error?.message || "切换运行模式失败，已回滚配置",
        };
      }

      return { success: true };
    },
    {
      body: t.Object({
        run_type: t.Union([t.Literal(0), t.Literal(1), t.Literal(3)]),
      }),
    },
  )
  .post(
    "/firewall/reset",
    async ({ body, set }) => {
      try {
        const result = await firewallService.resetFirewallForRunType(
          body.run_type,
        );
        const whitelistMessage =
          body.run_type === 0
            ? `，并同步 ${result.whitelistSynced} 条白名单 IP`
            : "";
        const exemptPortsMessage =
          body.run_type === 0 || body.run_type === 3
            ? `，保留入口端口 ${result.exemptPorts.join("、")}`
            : "";

        return {
          success: true,
          data: result,
          message: `已按${getRunTypeLabel(body.run_type)}重设防火墙${whitelistMessage}${exemptPortsMessage}`,
        };
      } catch (error: any) {
        set.status = 502;
        return {
          success: false,
          message: error?.message || "重设防火墙失败",
        };
      }
    },
    {
      body: t.Object({
        run_type: t.Union([t.Literal(0), t.Literal(1), t.Literal(3)]),
      }),
    },
  )
  .post("/firewall/clear", async ({ set }) => {
    try {
      const result = await firewallService.clearFirewall();
      return {
        success: true,
        data: result,
        message: `已清空防火墙规则，并移除 ${result.gatewayPort} 端口相关的历史重定向`,
      };
    } catch (error: any) {
      set.status = 502;
      return {
        success: false,
        message: error?.message || "清空防火墙失败",
      };
    }
  })
  .get("/config/run_mode_prompt_preferences", async () => {
    const preferences = await configManager.getRunModePromptPreferences();
    return { success: true, data: preferences };
  })
  .post(
    "/config/run_mode_prompt_preferences",
    async ({ body }) => {
      const patch: Partial<RunModePromptPreferences> = {};

      if (body.directToReverseProxy !== undefined) {
        patch.directToReverseProxy = body.directToReverseProxy;
      }
      if (body.reverseProxyToDirect !== undefined) {
        patch.reverseProxyToDirect = body.reverseProxyToDirect;
      }
      if (body.switchToSubdomain !== undefined) {
        patch.switchToSubdomain = body.switchToSubdomain;
      }
      if (body.subdomainToReverseProxy !== undefined) {
        patch.subdomainToReverseProxy = body.subdomainToReverseProxy;
      }

      const preferences =
        await configManager.updateRunModePromptPreferences(patch);
      return { success: true, data: preferences };
    },
    {
      body: t.Object({
        directToReverseProxy: t.Optional(t.Boolean()),
        reverseProxyToDirect: t.Optional(t.Boolean()),
        switchToSubdomain: t.Optional(t.Boolean()),
        subdomainToReverseProxy: t.Optional(t.Boolean()),
      }),
    },
  )
  .get("/config/protocol_mapping_feature", async () => {
    const settings = await configManager.getProtocolMappingFeatureConfig();
    return { success: true, data: settings };
  })
  .post(
    "/config/protocol_mapping_feature",
    async ({ body, set }) => {
      const [previousConfig, previousSettings] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      if (body.enabled === true && previousConfig.run_type !== 3) {
        set.status = 400;
        return {
          success: false,
          message: "协议映射仅可在子域模式下启用",
        };
      }
      try {
        const next =
          await configManager.updateProtocolMappingFeatureConfig(body);
        if (next.enabled === false) {
          await configManager.updateStreamMappings([]);
        }
        await firewallService.applyRunTypeConfig(
          previousConfig.run_type,
          previousConfig.run_type,
        );
        return { success: true, data: next };
      } catch (error: any) {
        const rollbackError = await rollbackProtocolMappingFeatureAndRuntime(
          previousSettings,
          previousConfig,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新协议映射功能开关失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新协议映射功能开关失败，已回滚配置",
        };
      }
    },
    {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    },
  )
  .get("/config/fnos_share_bypass", async () => {
    const settings = await configManager.getFnosShareBypassConfig();
    return { success: true, data: settings };
  })
  .post(
    "/config/fnos_share_bypass",
    async ({ body }) => {
      const next = await configManager.updateFnosShareBypassConfig(body);
      return { success: true, data: next };
    },
    {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        upstream_timeout_ms: t.Optional(t.Number()),
        validation_cache_ttl_seconds: t.Optional(t.Number()),
        validation_lock_ttl_seconds: t.Optional(t.Number()),
        session_ttl_seconds: t.Optional(t.Number()),
      }),
    },
  )
  .get("/config/gateway", async () => {
    const config = await configManager.getConfig();
    return {
      success: true,
      data: buildGatewaySettingsResponse(config),
    };
  })
  .post(
    "/config/gateway",
    async ({ body, set }) => {
      const previousConfig = await configManager.getConfig();

      try {
        const nextAuthConfigPatch: Partial<AppConfig["subdomain_mode"]> = {};
        if (body.auth_cache_ttl_seconds !== undefined) {
          nextAuthConfigPatch.auth_cache_ttl_seconds =
            body.auth_cache_ttl_seconds;
        }
        if (body.auth_cache_unauthorized_ttl_seconds !== undefined) {
          nextAuthConfigPatch.auth_cache_unauthorized_ttl_seconds =
            body.auth_cache_unauthorized_ttl_seconds;
        }

        if (Object.keys(nextAuthConfigPatch).length > 0) {
          await configManager.updateSubdomainModeConfig(nextAuthConfigPatch);
        }

        if (body.reverse_proxy_throttle) {
          await configManager.updateReverseProxyThrottleConfig(
            body.reverse_proxy_throttle,
          );
        }

        const updatedConfig = await configManager.getConfig();
        const [authConfigResult, reverseProxyThrottleResult] =
          await Promise.all([
            goBackend.setAuthConfig(buildGatewayAuthConfig(updatedConfig)),
            goBackend.setReverseProxyThrottle(
              updatedConfig.reverse_proxy_throttle ??
                DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
            ),
          ]);

        const syncErrors: string[] = [];
        if (!authConfigResult.success) {
          syncErrors.push(
            authConfigResult.message || "同步鉴权缓存配置到网关失败",
          );
        }
        if (!reverseProxyThrottleResult.success) {
          syncErrors.push(
            reverseProxyThrottleResult.message || "同步网关节流配置到网关失败",
          );
        }
        if (syncErrors.length > 0) {
          throw new Error(syncErrors.join("；"));
        }

        return {
          success: true,
          data: buildGatewaySettingsResponse(updatedConfig),
        };
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(previousConfig);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新网关配置失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新网关配置失败，已回滚配置",
        };
      }
    },
    {
      body: t.Object({
        auth_cache_ttl_seconds: t.Optional(t.Number()),
        auth_cache_unauthorized_ttl_seconds: t.Optional(t.Number()),
        reverse_proxy_throttle: t.Optional(
          t.Object({
            enabled: t.Optional(t.Boolean()),
            requests_per_second: t.Optional(t.Number()),
            burst: t.Optional(t.Number()),
            block_seconds: t.Optional(t.Number()),
          }),
        ),
      }),
    },
  )
  .get("/config/captcha", async () => {
    const settings = await configManager.getCaptchaSettings();
    return { success: true, data: settings };
  })
  .post(
    "/config/captcha",
    async ({ body, set }) => {
      if (body.provider === "turnstile") {
        const siteKey = body.turnstile?.site_key?.trim() || "";
        const secretKey = body.turnstile?.secret_key?.trim() || "";
        if (!siteKey || !secretKey) {
          set.status = 400;
          return {
            success: false,
            message:
              "启用 Cloudflare Turnstile 时，site_key 和 secret_key 都必须填写",
          };
        }
      }

      const next = await configManager.updateCaptchaSettings({
        provider: body.provider,
        turnstile: body.turnstile,
      });
      return { success: true, data: next };
    },
    {
      body: t.Object({
        provider: t.Union([t.Literal("pow"), t.Literal("turnstile")]),
        turnstile: t.Object({
          site_key: t.String(),
          secret_key: t.String(),
        }),
      }),
    },
  )
  .get("/config/terminal_feature", async () => {
    const settings = await configManager.getTerminalFeatureConfig();
    return { success: true, data: settings };
  })
  .get("/config/auth_credential_settings", async () => {
    const settings = await configManager.getAuthCredentialSettings();
    return { success: true, data: settings };
  })
  .post(
    "/config/auth_credential_settings",
    async ({ body }) => {
      const next = await configManager.updateAuthCredentialSettings(body);
      return { success: true, data: next };
    },
    {
      body: t.Object({
        session_ttl_seconds: t.Optional(t.Number()),
        remember_me_ttl_seconds: t.Optional(t.Number()),
      }),
    },
  )
  .post(
    "/config/terminal_feature",
    async ({ body }) => {
      const next = await configManager.updateTerminalFeatureConfig(body);
      return { success: true, data: next };
    },
    {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        default_shell: t.Optional(t.String()),
        default_cwd: t.Optional(t.String()),
        max_sessions: t.Optional(t.Number()),
        idle_timeout_seconds: t.Optional(t.Number()),
        resume_backend: t.Optional(t.Literal("tmux")),
        allow_mobile_toolbar: t.Optional(t.Boolean()),
        dangerously_run_as_current_user: t.Optional(t.Boolean()),
      }),
    },
  )
  .get("/config/default_route", async () => {
    const config = await configManager.getConfig();
    return { success: true, data: { default_route: config.default_route } };
  })
  .post(
    "/config/default_route",
    async ({ body }) => {
      await configManager.updateDefaultRoute(body.path);
      await goBackend.setDefaultRoute(body.path);
      return { success: true };
    },
    {
      body: t.Object({
        path: t.String(),
      }),
    },
  )
  .post(
    "/config/default_tunnel",
    async ({ body }) => {
      await configManager.updateDefaultTunnel(body.tunnel);
      return { success: true };
    },
    {
      body: t.Object({
        tunnel: t.Union([t.Literal("frp"), t.Literal("cloudflared")]),
      }),
    },
  )
  .post(
    "/config/proxy_mappings",
    async ({ body }) => {
      await configManager.updateProxyMappings(body.mappings);
      await goBackend.setRules(body.mappings);
      return { success: true };
    },
    {
      body: t.Object({
        mappings: t.Array(
          t.Object({
            path: t.String(),
            target: t.String(),
            rewrite_html: t.Boolean(),
            use_auth: t.Boolean(),
            use_root_mode: t.Boolean(),
            strip_path: t.Boolean(),
          }),
        ),
      }),
    },
  )
  .get("/config/host_mappings", async () => {
    const config = await configManager.getConfig();
    return { success: true, data: config.host_mappings };
  })
  .post(
    "/config/host_mappings",
    async ({ body, set }) => {
      const validation = validateHostMappings(body.mappings);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const config = await configManager.getConfig();
      const normalizedMappings: HostMapping[] = body.mappings.map(
        (mapping) => ({
          ...mapping,
          service_role: isAuthServiceTarget(mapping.target) ? "auth" : "app",
          title: mapping.title ?? "",
          title_override: mapping.title_override ?? "",
          favicon: mapping.favicon ?? "",
        }),
      );
      const nextConfig = {
        ...config,
        host_mappings: normalizedMappings,
      };
      const passkeyValidation = validatePasskeyRpConfig(nextConfig);
      if (!passkeyValidation.valid) {
        set.status = 400;
        return {
          success: false,
          message: passkeyValidation.message,
        };
      }

      const { mappings: enrichedMappings } =
        await enrichHostMappingsMetadataOnSave(
          normalizedMappings,
          config.host_mappings,
        );

      await configManager.updateHostMappings(enrichedMappings);
      const updatedConfig = await configManager.getConfig();
      await Promise.all([
        goBackend.setHostRules(updatedConfig.host_mappings),
        goBackend.setAuthConfig(buildGatewayAuthConfig(updatedConfig)),
      ]);
      return { success: true };
    },
    {
      body: t.Object({
        mappings: t.Array(
          t.Object({
            host: t.String(),
            target: t.String(),
            use_auth: t.Boolean(),
            access_mode: t.Union([
              t.Literal("login_first"),
              t.Literal("strict_whitelist"),
            ]),
            suppress_toolbar: t.Boolean(),
            preserve_host: t.Boolean(),
            service_role: t.Optional(
              t.Union([t.Literal("app"), t.Literal("auth")]),
            ),
            title: t.Optional(t.String()),
            title_override: t.Optional(t.String()),
            favicon: t.Optional(t.String()),
          }),
        ),
      }),
    },
  )
  .post("/config/host_mappings/refresh_titles", async () => {
    const config = await configManager.getConfig();
    const { mappings, summary } = await refreshAllHostMappingTitles(
      config.host_mappings,
    );

    await configManager.updateHostMappings(mappings);

    return {
      success: true,
      data: summary,
    };
  })
  .get("/config/host_mappings/bookmarks/export", async ({ request }) => {
    const config = await configManager.getConfig();
    const document = buildHostMappingsBookmarksDocument({
      mappings: config.host_mappings,
      scheme: resolveRequestScheme(request),
      accessEntryPort: process.env.GO_REPROXY_PORT || "7999",
      folderTitle: config.subdomain_mode?.root_domain?.trim()
        ? `${config.subdomain_mode.root_domain.trim()} 子域映射`
        : "fn-knock 子域映射",
    });
    const filename = buildHostMappingsBookmarkFilename(
      config.subdomain_mode?.root_domain,
    );
    const body = new Blob([document], {
      type: "text/html;charset=UTF-8",
    });

    return new Response(body, {
      headers: {
        "Content-Type": "text/html; charset=UTF-8",
        "Content-Disposition": `attachment; filename="${filename}"`,
        "Cache-Control": "no-store",
      },
    });
  })
  .get("/config/stream_mappings", async () => {
    const config = await configManager.getConfig();
    return { success: true, data: config.stream_mappings };
  })
  .post(
    "/config/stream_mappings",
    async ({ body, set }) => {
      const validation = validateStreamMappings(body.mappings);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const previousConfig = await configManager.getConfig();
      await configManager.updateStreamMappings(body.mappings);
      const updatedConfig = await configManager.getConfig();
      try {
        await firewallService.applyRunTypeConfig(
          updatedConfig.run_type,
          updatedConfig.run_type,
        );
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(previousConfig);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "同步 协议映射与网关端口放行规则失败"}；回滚失败：${rollbackError}`
            : error?.message ||
              "同步 协议映射与网关端口放行规则失败，已回滚配置",
        };
      }

      return { success: true };
    },
    {
      body: t.Object({
        mappings: t.Array(
          t.Object({
            protocol: t.Optional(t.Union([t.Literal("tcp"), t.Literal("udp")])),
            listen_port: t.Number(),
            target: t.String(),
            use_auth: t.Boolean(),
          }),
        ),
      }),
    },
  )
  .get("/config/subdomain_mode", async () => {
    const config = await configManager.getConfig();
    return { success: true, data: config.subdomain_mode };
  })
  .post(
    "/config/subdomain_mode",
    async ({ body, set }) => {
      const config = await configManager.getConfig();
      const nextConfig = {
        ...config,
        subdomain_mode: {
          ...config.subdomain_mode,
          ...body,
        },
      };
      const validation = validateHostMappings(nextConfig.host_mappings);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const passkeyValidation = validatePasskeyRpConfig(nextConfig);
      if (!passkeyValidation.valid) {
        set.status = 400;
        return {
          success: false,
          message: passkeyValidation.message,
        };
      }

      const next = await configManager.updateSubdomainModeConfig(body);
      const updatedConfig = await configManager.getConfig();
      await goBackend.setAuthConfig(buildGatewayAuthConfig(updatedConfig));

      const sslStatus = await configManager.getSSLStatus();
      const inventoryCoverage = buildSubdomainCertificateInventoryCoverage({
        config: updatedConfig,
        certificates: sslStatus.certificates.map((certificate) => ({
          id: certificate.id,
          certificateDomains: certificate.certInfo?.dnsNames || [],
        })),
        activeCertificateId: sslStatus.activeCertId,
        deploymentMode: sslStatus.deploymentMode,
      });

      let sslAutoSelection: {
        applied: boolean;
        certificate_id?: string;
        label?: string;
        message: string;
      } | null = null;

      if (
        inventoryCoverage.can_auto_activate &&
        inventoryCoverage.suggested_certificate_id
      ) {
        const previousActiveId = sslStatus.activeCertId || null;
        const candidate = await configManager.activateSSLCertificate(
          inventoryCoverage.suggested_certificate_id,
        );

        if (candidate) {
          try {
            await syncSSLDeploymentToGateway();
            sslAutoSelection = {
              applied: true,
              certificate_id: candidate.id,
              label: candidate.label,
              message: "已自动切换到更适合当前子域模式的证书。",
            };
          } catch (error: any) {
            await configManager.activateSSLCertificate(previousActiveId);
            await syncSSLDeploymentToGateway().catch(() => undefined);

            sslAutoSelection = {
              applied: false,
              certificate_id: candidate.id,
              label: candidate.label,
              message:
                error?.message ||
                "已找到推荐证书，但同步到网关失败，未自动切换。",
            };
          }
        }
      }

      return {
        success: true,
        data: {
          ...next,
          ssl_auto_selection: sslAutoSelection,
        },
      };
    },
    {
      body: t.Object({
        root_domain: t.Optional(t.String()),
        auth_host: t.Optional(t.String()),
        auth_target: t.Optional(t.String()),
        cookie_domain: t.Optional(t.String()),
        public_auth_base_url: t.Optional(t.String()),
        public_http_port: t.Optional(t.Number()),
        public_https_port: t.Optional(t.Number()),
        auth_cache_ttl_seconds: t.Optional(t.Number()),
        auth_cache_unauthorized_ttl_seconds: t.Optional(t.Number()),
        default_access_mode: t.Optional(
          t.Union([t.Literal("login_first"), t.Literal("strict_whitelist")]),
        ),
        auto_add_whitelist_on_login: t.Optional(t.Boolean()),
        passkey_rp_mode: t.Optional(
          t.Union([t.Literal("auth_host"), t.Literal("parent_domain")]),
        ),
        passkey_rp_id: t.Optional(t.String()),
      }),
    },
  )
  // TOTP 认证管理
  .get("/totp/status", async () => {
    const credentials = await configManager.getTOTPCredentials();
    return {
      success: true,
      data: { bound: credentials.length > 0, credentials },
    };
  })
  .post("/totp/setup", async () => {
    const secret = generateSecret();
    const uri = generateURI({
      issuer: "fn-knock",
      label: "admin",
      secret,
      strategy: "totp",
    });
    return { success: true, data: { secret, uri } };
  })
  .post(
    "/totp/bind",
    async ({ body, set }) => {
      const { valid } = verifySync({
        strategy: "totp",
        token: body.token,
        secret: body.secret,
      });
      if (!valid) {
        set.status = 400;
        return { success: false, message: "验证码不正确，请重试" };
      }
      await configManager.addTOTPCredential({
        id: randomBytes(8).toString("hex"),
        secret: body.secret,
        comment: body.comment || "New Token",
        createdAt: new Date().toISOString(),
      });
      return { success: true };
    },
    {
      body: t.Object({
        secret: t.String(),
        token: t.String(),
        comment: t.Optional(t.String()),
      }),
    },
  )
  .delete(
    "/totp/:id",
    async ({ params, set }) => {
      const deleted = await configManager.deleteTOTPCredential(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: "TOTP not found" };
      }
      return { success: true };
    },
    {
      params: t.Object({ id: t.String() }),
    },
  )
  .patch(
    "/totp/:id/comment",
    async ({ params, body, set }) => {
      const updated = await configManager.updateTOTPCredential(
        params.id,
        body.comment,
      );
      if (!updated) {
        set.status = 404;
        return { success: false, message: "TOTP not found" };
      }
      return { success: true };
    },
    {
      params: t.Object({ id: t.String() }),
      body: t.Object({ comment: t.String() }),
    },
  )
  .get(
    "/totp/:totpId/passkeys",
    async ({ params }) => {
      const passkeys = await configManager.getPasskeys();
      const filtered = passkeys.filter((pk) => pk.totpId === params.totpId);
      return { success: true, data: filtered };
    },
    {
      params: t.Object({ totpId: t.String() }),
    },
  )
  .delete(
    "/passkeys/:id",
    async ({ params, set }) => {
      const deleted = await configManager.deletePasskey(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: "Passkey not found" };
      }
      return { success: true };
    },
    {
      params: t.Object({
        id: t.String(),
      }),
    },
  )
  .post("/sync-routes", async ({ set }) => {
    try {
      const [config, protocolMappingFeature] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      await firewallService.applyRunTypeConfig(
        config.run_type,
        config.run_type,
      );

      const loggingResult = await goBackend.setGatewayLoggingConfig(
        config.gateway_logging ?? {
          enabled: false,
          max_days: 7,
        },
      );
      if (!loggingResult.success) {
        set.status = 502;
        return {
          success: false,
          message: `同步部分失败: gateway_logging=${loggingResult.success}`,
        };
      }

      const syncedRules =
        config.run_type === 1 ? config.proxy_mappings.length : 0;
      const syncedHostRules =
        config.run_type === 3 ? config.host_mappings.length : 0;
      const syncedStreamRules =
        config.run_type === 3 && protocolMappingFeature.enabled === true
          ? config.stream_mappings.length
          : 0;

      return {
        success: true,
        data: {
          synced_rules: syncedRules,
          synced_host_rules: syncedHostRules,
          synced_stream_rules: syncedStreamRules,
          synced_gateway_logging: true,
        },
        message: `已按当前运行模式同步 ${syncedRules} 条路径路由、${syncedHostRules} 条 Host 路由、${syncedStreamRules} 条 协议映射与请求日志配置`,
      };
    } catch (e: any) {
      set.status = 500;
      return { success: false, message: e?.message ?? String(e) };
    }
  })
  .get("/maintenance/backup/export", async () => {
    const archive = await maintenanceBackupService.exportBackupArchive();
    const body = new Blob([Uint8Array.from(archive.buffer)], {
      type: "application/octet-stream",
    });

    return new Response(body, {
      headers: {
        "Content-Type": "application/octet-stream",
        "Content-Disposition": `attachment; filename="${archive.filename}"`,
        "Cache-Control": "no-store",
      },
    });
  })
  .get("/maintenance/backup/files", async ({ set }) => {
    try {
      return {
        success: true,
        data: await maintenanceBackupService.listBackupDirectoryFiles(),
      };
    } catch (error: any) {
      set.status = 500;
      return {
        success: false,
        message: error?.message || "读取飞牛备份目录失败",
      };
    }
  })
  .post("/maintenance/backup/export/fnos", async ({ set }) => {
    try {
      const result =
        await maintenanceBackupService.exportBackupArchiveToDirectory();
      return {
        success: true,
        data: result,
        message: "备份已导出到飞牛目录",
      };
    } catch (error: any) {
      const status =
        error instanceof MaintenanceBackupError ? error.status : 500;
      set.status = status;
      return {
        success: false,
        message: error?.message || "导出到飞牛目录失败",
      };
    }
  })
  .post(
    "/maintenance/backup/import",
    async ({ body, set }) => {
      try {
        const result = await maintenanceBackupService.importBackupArchive(body);
        return {
          success: true,
          data: result,
          message:
            result.warnings.length > 0
              ? "备份已导入，但部分运行态同步失败"
              : "备份已导入并完成运行态同步",
        };
      } catch (error: any) {
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status = status;
        return {
          success: false,
          message: error?.message || "导入备份失败",
        };
      }
    },
    {
      body: t.Object({
        filename: t.Optional(t.String()),
        archive_base64: t.String(),
      }),
    },
  )
  .post(
    "/maintenance/backup/import/fnos",
    async ({ body, set }) => {
      try {
        const result =
          await maintenanceBackupService.importBackupArchiveFromDirectory(
            body.path,
          );
        return {
          success: true,
          data: result,
          message:
            result.warnings.length > 0
              ? "飞牛备份已导入，但部分运行态同步失败"
              : "飞牛备份已导入并完成运行态同步",
        };
      } catch (error: any) {
        const message = error?.message || "从飞牛导入备份失败";
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status =
          error?.code === "ENOENT"
            ? 404
            : error?.code === "EACCES"
              ? 403
              : status;
        return {
          success: false,
          message,
        };
      }
    },
    {
      body: t.Object({
        path: t.String(),
      }),
    },
  )
  .get(
    "/logs",
    async ({ query }) => {
      const page = parseInt(query.page || "1", 10);
      const limit = parseInt(query.limit || "50", 10);
      const search = query.search || "";
      const result = await authLogManager.getLogs(page, limit, search);
      return { success: true, data: result };
    },
    {
      query: t.Object({
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        search: t.Optional(t.String()),
      }),
    },
  )
  .delete(
    "/logs",
    async ({ body }) => {
      await authLogManager.deleteLogs(body.ids);
      return { success: true };
    },
    {
      body: t.Object({
        ids: t.Array(t.String()),
      }),
    },
  )
  .get(
    "/security/overview",
    async ({ query }) => {
      const rangeSec = clamp(
        parseIntSafe(query.rangeSec, 3600),
        60,
        30 * 24 * 3600,
      );
      const nowMs = Date.now();
      const fromMs = nowMs - rangeSec * 1000;
      const bucketCount = Math.min(
        48,
        Math.max(12, Math.round(rangeSec / 900)),
      );
      const logs = await authLogManager.listLogsByRange(fromMs, nowMs);
      const failedTimestamps = logs
        .filter((item) => item.log.type === "login" && !item.log.success)
        .map((item) => item.timestamp);
      const blockedRecords = await scanDetector.listBlacklistByRange(
        fromMs,
        nowMs,
      );
      const blockedTimestamps = blockedRecords.map((item) => item.blockedAt);

      return {
        success: true,
        data: {
          rangeSec,
          totals: {
            failedLogins: failedTimestamps.length,
            blockedScanners: blockedTimestamps.length,
          },
          series: {
            failedLogins: buildCountSeries(
              failedTimestamps,
              fromMs,
              nowMs,
              bucketCount,
            ),
            blockedScanners: buildCountSeries(
              blockedTimestamps,
              fromMs,
              nowMs,
              bucketCount,
            ),
          },
        },
      };
    },
    {
      query: t.Object({
        rangeSec: t.Optional(t.String()),
      }),
    },
  )
  // Session management
  .get("/sessions", async () => {
    const list = await configManager.listSessions();
    const mapped = await Promise.all(
      list.map(async ({ id, data }) => {
        const session = await ensureSessionComment(id, data);
        return {
          id,
          ...session,
          mobility:
            await authMobilitySessionManager.getSessionMobilitySummary(id),
        };
      }),
    );
    await ipLocationService.hydrateIpLocationRecords(mapped, (session) =>
      ipLocationRefs.session(session.id),
    );
    return { success: true, data: mapped };
  })
  .get(
    "/sessions/:id",
    async ({ params, set }) => {
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }
      const session = await ensureSessionComment(params.id, sess);
      const record = { id: params.id, ...session };
      await ipLocationService.hydrateIpLocationRecords([record], (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: record };
    },
    {
      params: t.Object({ id: t.String() }),
    },
  )
  .patch(
    "/sessions/:id/comment",
    async ({ params, body, set }) => {
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }

      const updated = await configManager.updateSession(params.id, {
        comment: body.comment,
      });
      if (!updated) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }

      const whitelistRecordId =
        await authMobilitySessionManager.getSessionWhitelistRecordId(params.id);
      if (whitelistRecordId) {
        await whitelistManager.updateComment(whitelistRecordId, body.comment);
      }

      const record = { id: params.id, ...updated };
      await ipLocationService.hydrateIpLocationRecords([record], (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: record };
    },
    {
      params: t.Object({ id: t.String() }),
      body: t.Object({ comment: t.String() }),
    },
  )
  .get(
    "/sessions/:id/mobility",
    async ({ params, set }) => {
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }
      const details =
        await authMobilitySessionManager.getSessionMobilityDetails(params.id);
      await ipLocationService.hydrateMobilityEvents(details.events, params.id);
      return {
        success: true,
        data: details,
      };
    },
    {
      params: t.Object({ id: t.String() }),
    },
  )
  .delete(
    "/sessions/:id",
    async ({ params }) => {
      const sess = await configManager.getSession(params.id);
      if (sess) {
        await authMobilitySessionManager.destroySession(params.id);
        await configManager.deleteSession(params.id);
        await authLogManager.recordLog({
          type: "logout",
          ip: sess.ip,
          userAgent: sess.userAgent,
          success: true,
        });
      } else {
        await configManager.deleteSession(params.id);
      }
      return { success: true };
    },
    {
      params: t.Object({ id: t.String() }),
    },
  );
