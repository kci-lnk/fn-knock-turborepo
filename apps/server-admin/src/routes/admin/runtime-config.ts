import { Elysia, t } from "elysia";
import {
  DEFAULT_IP_LOCATION_API_CONFIG,
  type AppConfig,
  configManager,
  type IpLocationApiMode,
  redis,
  type RunModePromptPreferences,
} from "../../lib/redis";
import { goBackend } from "../../lib/go-backend";
import { firewallService } from "../../lib/firewall-service";
import { scheduleSyncReverseProxyTrustedIPs } from "../../lib/reverse-proxy-trusted-ips";
import { whitelistManager } from "../../lib/whitelist-manager";
import { getGatewayLoggingConfigForResponse } from "../../lib/gateway-logging";
import { syncWAFConfigToGateway } from "../../lib/waf/service";
import {
  getSmartConnectDetails,
  syncSmartConnect,
} from "../../lib/smart-connect";
import {
  isAnySubdomainRoutingMode,
  isReverseProxySubdomainMode,
} from "../../lib/reverse-proxy-submode";
import {
  getRuntimeCapabilities,
  getRuntimeProfile,
} from "../../lib/runtime-profile";
import { buildIpLocationApiUrl } from "../../lib/ip-location-api-url";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import {
  autoHttpsRedirectManager,
  type AutoHttpsConfig,
} from "../../lib/auto-https-redirect";
import { normalizeLocaleConfig } from "../../../../../packages/i18n/src";
import { normalizeAppearanceConfig } from "../../../../../packages/admin-shared/src/utils/appearance";
import { validateIpLocationBaseUrl } from "./validation";
import {
  adminT,
  buildCapabilityBlockedResponse,
  ensureGoResponseSuccess,
  getAdminRouteTranslator,
  getRunTypeLabel,
  rollbackConfigAndRuntime,
  rollbackProtocolMappingFeatureAndRuntime,
} from "./shared";

const buildAutoHttpsDetails = async (settings?: AutoHttpsConfig) => {
  const config = settings ?? (await configManager.getAutoHttpsConfig());
  return {
    ...config,
    runtime: autoHttpsRedirectManager.getRuntimeState(),
  };
};

export const adminRuntimeConfigRoutes = new Elysia()
  .get(
    "/healthz",
    async ({ set }) => {
      let redisReachable = false;
      let redisError: string | null = null;

      try {
        redisReachable = (await redis.ping()) === "PONG";
      } catch (error) {
        redisError =
          error instanceof Error ? error.message : "Redis is unavailable";
      }

      const gatewayProbe = await goBackend.getServerInfo();
      const isHealthy = redisReachable && gatewayProbe.success;

      if (!isHealthy) {
        set.status = 503;
      }

      return {
        success: isHealthy,
        data: {
          node: {
            alive: true,
            pid: process.pid,
          },
          redis: {
            reachable: redisReachable,
            error: redisError,
          },
          runtime_profile: getRuntimeProfile(),
          gateway_admin: {
            reachable: gatewayProbe.success,
            version: gatewayProbe.data?.version ?? null,
            error:
              gatewayProbe.success === true
                ? null
                : gatewayProbe.message || "Gateway admin probe failed",
          },
        },
      };
    },
    routeDoc("获取运行时健康检查状态"),
  )
  .get(
    "/config",
    async () => {
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
    },
    routeDoc("获取管理端完整配置"),
  )
  .get(
    "/config/locale",
    async () => {
      const locale = await configManager.getLocaleConfig();
      return { success: true, data: locale };
    },
    routeDoc("获取语言配置"),
  )
  .post(
    "/config/locale",
    async ({ body }) => {
      const next = normalizeLocaleConfig(body);
      const saved = await configManager.updateLocaleConfig(next);
      const gatewayResponse = await goBackend.setLocaleConfig(saved);
      if (!gatewayResponse.success && gatewayResponse.code !== 404) {
        console.warn(
          "[i18n] failed to sync locale config to Go gateway:",
          gatewayResponse.message,
        );
      }
      return { success: true, data: saved };
    },
    withRouteDoc("更新语言配置", {
      body: t.Object({
        default_locale: t.Union([
          t.Literal("zh-CN"),
          t.Literal("zh-Hant"),
          t.Literal("en"),
          t.Literal("ko-KR"),
          t.Literal("ja-JP"),
        ]),
      }),
    }),
  )
  .get(
    "/config/appearance",
    async () => {
      const appearance = await configManager.getAppearanceConfig();
      return { success: true, data: appearance };
    },
    routeDoc("获取后台外观配置"),
  )
  .post(
    "/config/appearance",
    async ({ body }) => {
      const next = normalizeAppearanceConfig(body);
      const saved = await configManager.updateAppearanceConfig(next);
      return { success: true, data: saved };
    },
    withRouteDoc("更新后台外观配置", {
      body: t.Object({
        theme_color_preset: t.Optional(
          t.Union([
            t.Literal("default"),
            t.Literal("hermes_orange"),
            t.Literal("prussian_blue"),
            t.Literal("dynamic_white"),
          ]),
        ),
      }),
    }),
  )
  .post(
    "/config/run_type",
    async ({ request, body, set }) => {
      const { locale, t } = await getAdminRouteTranslator(request);
      if (
        body.run_type === 0 &&
        !getRuntimeCapabilities().direct_mode_available
      ) {
        return buildCapabilityBlockedResponse(set, "direct_mode_available");
      }

      const [config, previousProtocolMappingFeature] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      const previousRunType = config.run_type;
      try {
        await configManager.updateRunType(
          body.run_type,
          body.reverse_proxy_submode,
        );
        if (body.run_type !== 3) {
          await configManager.updateProtocolMappingFeatureConfig({
            enabled: false,
          });
        }
        await syncSmartConnect(await configManager.getConfig(), locale);
        await firewallService.applyRunTypeConfig(
          body.run_type,
          previousRunType,
        );
        if (body.run_type === 0) {
          try {
            const removedAutoGrantCount =
              await whitelistManager.removeRecordsBySource("auto");
            if (removedAutoGrantCount > 0) {
              scheduleSyncReverseProxyTrustedIPs({
                reason: "run-type-direct-cleanup",
              });
            }
          } catch (cleanupError) {
            console.error(
              "[admin][run_type] failed to clear login IP grants after switching to direct mode:",
              cleanupError,
            );
          }
        }
      } catch (error: any) {
        const rollbackError = await rollbackProtocolMappingFeatureAndRuntime(
          previousProtocolMappingFeature,
          config,
          t,
          locale,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message: error?.message || adminT(t, "runType.switchFailed"),
                rollbackError,
              })
            : error?.message || adminT(t, "runType.switchFailedRolledBack"),
        };
      }

      return { success: true };
    },
    withRouteDoc("切换运行模式", {
      body: t.Object({
        run_type: t.Union([t.Literal(0), t.Literal(1), t.Literal(3)]),
        reverse_proxy_submode: t.Optional(
          t.Union([t.Literal("path"), t.Literal("subdomain")]),
        ),
      }),
    }),
  )
  .post(
    "/config/auto_manage_firewall",
    async ({ body, set }) => {
      if (!getRuntimeCapabilities().host_firewall_available) {
        return buildCapabilityBlockedResponse(set, "host_firewall_available");
      }

      const next = await configManager.updateAutoManageFirewall(
        body.auto_manage_firewall,
      );
      return {
        success: true,
        data: {
          auto_manage_firewall: next,
        },
      };
    },
    withRouteDoc("更新防火墙自动管理开关", {
      body: t.Object({
        auto_manage_firewall: t.Boolean(),
      }),
    }),
  )
  .post(
    "/firewall/reset",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      if (!getRuntimeCapabilities().host_firewall_available) {
        return buildCapabilityBlockedResponse(set, "host_firewall_available");
      }
      if (
        body.run_type === 0 &&
        !getRuntimeCapabilities().direct_mode_available
      ) {
        return buildCapabilityBlockedResponse(set, "direct_mode_available");
      }

      try {
        const result = await firewallService.resetFirewallForRunType(
          body.run_type,
        );
        const whitelistMessage =
          body.run_type === 0
            ? adminT(t, "firewall.whitelistSynced", {
                count: result.whitelistSynced,
              })
            : "";
        const exemptPortsMessage =
          body.run_type === 0 || body.run_type === 3
            ? adminT(t, "firewall.exemptPorts", {
                ports: result.exemptPorts.join(", "),
              })
            : "";

        return {
          success: true,
          data: result,
          message: adminT(t, "firewall.resetSuccess", {
            exemptPortsMessage,
            runType: getRunTypeLabel(t, body.run_type),
            whitelistMessage,
          }),
        };
      } catch (error: any) {
        set.status = 502;
        return {
          success: false,
          message: error?.message || adminT(t, "firewall.resetFailed"),
        };
      }
    },
    withRouteDoc("按运行模式重置防火墙", {
      body: t.Object({
        run_type: t.Union([t.Literal(0), t.Literal(1), t.Literal(3)]),
      }),
    }),
  )
  .post(
    "/firewall/clear",
    async ({ request, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      if (!getRuntimeCapabilities().host_firewall_available) {
        return buildCapabilityBlockedResponse(set, "host_firewall_available");
      }

      try {
        const result = await firewallService.clearFirewall();
        return {
          success: true,
          data: result,
          message: adminT(t, "firewall.clearSuccess", {
            port: result.gatewayPort,
          }),
        };
      } catch (error: any) {
        set.status = 502;
        return {
          success: false,
          message: error?.message || adminT(t, "firewall.clearFailed"),
        };
      }
    },
    routeDoc("清空防火墙规则"),
  )
  .get(
    "/config/run_mode_prompt_preferences",
    async () => {
      const preferences = await configManager.getRunModePromptPreferences();
      return { success: true, data: preferences };
    },
    routeDoc("获取运行模式提示偏好"),
  )
  .get(
    "/config/welcome_guide",
    async () => {
      const status = await configManager.getWelcomeGuideStatus();
      return { success: true, data: status };
    },
    routeDoc("获取欢迎向导状态"),
  )
  .post(
    "/config/welcome_guide/complete",
    async () => {
      const status = await configManager.completeWelcomeGuide();
      return { success: true, data: status };
    },
    routeDoc("完成欢迎向导"),
  )
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
    withRouteDoc("更新运行模式提示偏好", {
      body: t.Object({
        directToReverseProxy: t.Optional(t.Boolean()),
        reverseProxyToDirect: t.Optional(t.Boolean()),
        switchToSubdomain: t.Optional(t.Boolean()),
        subdomainToReverseProxy: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/protocol_mapping_feature",
    async () => {
      const settings = await configManager.getProtocolMappingFeatureConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取协议映射功能开关"),
  )
  .post(
    "/config/protocol_mapping_feature",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const [previousConfig, previousSettings] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      if (body.enabled === true && previousConfig.run_type !== 3) {
        set.status = 400;
        return {
          success: false,
          message: adminT(t, "protocolMapping.subdomainOnly"),
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
          t,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message ||
                  adminT(t, "protocolMapping.updateFeatureFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "protocolMapping.updateFeatureFailedRolledBack"),
        };
      }
    },
    withRouteDoc("更新协议映射功能开关", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/smart_connect/details",
    async ({ request }) => {
      const { locale } = await getAdminRouteTranslator(request);
      const details = await getSmartConnectDetails(undefined, locale);
      return { success: true, data: details };
    },
    routeDoc("获取智能连接详情"),
  )
  .post(
    "/config/smart_connect",
    async ({ request, body, set }) => {
      const { locale, t } = await getAdminRouteTranslator(request);
      if (!getRuntimeCapabilities().smart_connect_available) {
        return buildCapabilityBlockedResponse(set, "smart_connect_available");
      }

      const previousConfig = await configManager.getConfig();
      if (body.enabled === true && previousConfig.run_type !== 3) {
        set.status = 400;
        return {
          success: false,
          message: adminT(t, "smartConnect.subdomainOnly"),
        };
      }

      const nextConfig: AppConfig = {
        ...previousConfig,
        smart_connect: {
          ...(previousConfig.smart_connect ?? {
            enabled: false,
            selected_ipv4: "",
          }),
          ...(body.enabled !== undefined ? { enabled: body.enabled } : {}),
          ...(body.selected_ipv4 !== undefined
            ? { selected_ipv4: body.selected_ipv4 }
            : {}),
        },
      };

      try {
        await configManager.saveConfig(nextConfig);
        const details = await syncSmartConnect(nextConfig, locale);
        await firewallService.applyRunTypeConfig(
          nextConfig.run_type,
          previousConfig.run_type,
        );
        return { success: true, data: details };
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(
          previousConfig,
          t,
          locale,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message || adminT(t, "smartConnect.updateFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "smartConnect.updateFailedRolledBack"),
        };
      }
    },
    withRouteDoc("更新智能连接配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        selected_ipv4: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/config/fnos_share_bypass",
    async () => {
      const settings = await configManager.getFnosShareBypassConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取飞牛共享绕过配置"),
  )
  .post(
    "/config/fnos_share_bypass",
    async ({ body }) => {
      const next = await configManager.updateFnosShareBypassConfig(body);
      return { success: true, data: next };
    },
    withRouteDoc("更新飞牛共享绕过配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        upstream_timeout_ms: t.Optional(t.Number()),
        validation_cache_ttl_seconds: t.Optional(t.Number()),
        validation_lock_ttl_seconds: t.Optional(t.Number()),
        session_ttl_seconds: t.Optional(t.Number()),
      }),
    }),
  )
  .get(
    "/config/fnos_port_icon_hijack",
    async () => {
      const settings = await configManager.getFnosPortIconHijackConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取飞牛端口图标接管配置"),
  )
  .post(
    "/config/fnos_port_icon_hijack",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const previousConfig = await configManager.getConfig();
      const next = await configManager.updateFnosPortIconHijackConfig(body);
      try {
        ensureGoResponseSuccess(
          await goBackend.setFnosPortIconHijackConfig(next),
          adminT(t, "fnosPortIcon.syncFailed"),
        );
      } catch (error: any) {
        let rollbackError: string | null = null;
        try {
          const rollbackConfig = await configManager.getConfig();
          rollbackConfig.fnos_port_icon_hijack =
            previousConfig.fnos_port_icon_hijack;
          await configManager.saveConfig(rollbackConfig);
        } catch (innerError: any) {
          rollbackError =
            innerError?.message || adminT(t, "rollback.restoreConfigFailed");
        }
        set.status = 502;
        const message = error?.message || adminT(t, "fnosPortIcon.syncFailed");
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", { message, rollbackError })
            : message,
        };
      }
      return { success: true, data: next };
    },
    withRouteDoc("更新飞牛端口图标接管配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/captcha",
    async () => {
      const settings = await configManager.getCaptchaSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取验证码配置"),
  )
  .post(
    "/config/captcha",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      if (body.provider === "turnstile") {
        const siteKey = body.turnstile?.site_key?.trim() || "";
        const secretKey = body.turnstile?.secret_key?.trim() || "";
        if (!siteKey || !secretKey) {
          set.status = 400;
          return {
            success: false,
            message: adminT(t, "captcha.turnstileKeysRequired"),
          };
        }
      }

      const next = await configManager.updateCaptchaSettings({
        provider: body.provider,
        turnstile: body.turnstile,
      });
      return { success: true, data: next };
    },
    withRouteDoc("更新验证码配置", {
      body: t.Object({
        provider: t.Union([t.Literal("pow"), t.Literal("turnstile")]),
        turnstile: t.Object({
          site_key: t.String(),
          secret_key: t.String(),
        }),
      }),
    }),
  )
  .get(
    "/config/ip_location_api",
    async () => {
      const settings = await configManager.getIpLocationApiSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取 IP 属地 API 配置"),
  )
  .post(
    "/config/ip_location_api",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const ipLookupMode: IpLocationApiMode = body.ip_lookup_mode;
      const cidrMode: IpLocationApiMode = body.cidr_mode;
      const ipLookupUrl =
        ipLookupMode === "custom"
          ? validateIpLocationBaseUrl(
              body.ip_lookup_url,
              adminT(t, "ipLocation.ipLookupUrlLabel"),
              t,
            )
          : {
              valid: true as const,
              url: DEFAULT_IP_LOCATION_API_CONFIG.ip_lookup_url,
            };
      const cidrUrl =
        cidrMode === "custom"
          ? validateIpLocationBaseUrl(
              body.cidr_url,
              adminT(t, "ipLocation.cidrUrlLabel"),
              t,
            )
          : {
              valid: true as const,
              url: DEFAULT_IP_LOCATION_API_CONFIG.cidr_url,
            };

      if (!ipLookupUrl.valid) {
        set.status = 400;
        return { success: false, message: ipLookupUrl.message };
      }
      if (!cidrUrl.valid) {
        set.status = 400;
        return { success: false, message: cidrUrl.message };
      }

      const next = await configManager.updateIpLocationApiSettings({
        ip_lookup_mode: ipLookupMode,
        ip_lookup_url: ipLookupUrl.url,
        cidr_mode: cidrMode,
        cidr_url: cidrUrl.url,
      });
      return { success: true, data: next };
    },
    withRouteDoc("更新 IP 属地 API 配置", {
      body: t.Object({
        ip_lookup_mode: t.Union([t.Literal("online"), t.Literal("custom")]),
        ip_lookup_url: t.String(),
        cidr_mode: t.Union([t.Literal("online"), t.Literal("custom")]),
        cidr_url: t.String(),
      }),
    }),
  )
  .post(
    "/config/ip_location_api/test-ip-lookup",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const validation = validateIpLocationBaseUrl(body.url, "URL", t);
      if (!validation.valid) {
        set.status = 400;
        return { success: false, message: validation.message };
      }

      const timeoutMs = 5000;

      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        const url = buildIpLocationApiUrl(validation.url, "ip/lookup");
        url.searchParams.set("ip", "8.8.8.8");

        const response = await fetch(url, {
          signal: controller.signal,
          headers: { "User-Agent": "fn-knock-server-admin/1.0" },
        });

        clearTimeout(timer);

        if (!response.ok) {
          return {
            success: false,
            message: adminT(t, "connectionTest.httpStatus", {
              status: response.status,
            }),
          };
        }

        const data = (await response.json().catch(() => null)) as {
          code?: number;
          result?: unknown;
          msg?: string;
        } | null;
        if (!data || data.code !== 0 || !data.result) {
          return {
            success: false,
            message: data?.msg || adminT(t, "connectionTest.invalidData"),
          };
        }

        return { success: true, message: adminT(t, "connectionTest.success") };
      } catch (error: any) {
        if (error?.name === "AbortError") {
          return {
            success: false,
            message: adminT(t, "connectionTest.timeout"),
          };
        }
        return {
          success: false,
          message: error?.message || adminT(t, "connectionTest.failed"),
        };
      }
    },
    withRouteDoc("测试 IP 识别库连接", {
      body: t.Object({
        url: t.String(),
      }),
    }),
  )
  .post(
    "/config/ip_location_api/test-cidr",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const validation = validateIpLocationBaseUrl(body.url, "URL", t);
      if (!validation.valid) {
        set.status = 400;
        return { success: false, message: validation.message };
      }

      const timeoutMs = 5000;

      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        const url = buildIpLocationApiUrl(validation.url, "provinces");

        const response = await fetch(url, {
          signal: controller.signal,
          headers: { "User-Agent": "fn-knock-server-admin/1.0" },
        });

        clearTimeout(timer);

        if (!response.ok) {
          return {
            success: false,
            message: adminT(t, "connectionTest.httpStatus", {
              status: response.status,
            }),
          };
        }

        const data = (await response.json().catch(() => null)) as {
          code?: number;
          data?: unknown;
          message?: string;
        } | null;
        if (!data || data.code !== 0 || !data.data) {
          return {
            success: false,
            message: data?.message || adminT(t, "connectionTest.invalidData"),
          };
        }

        return { success: true, message: adminT(t, "connectionTest.success") };
      } catch (error: any) {
        if (error?.name === "AbortError") {
          return {
            success: false,
            message: adminT(t, "connectionTest.timeout"),
          };
        }
        return {
          success: false,
          message: error?.message || adminT(t, "connectionTest.failed"),
        };
      }
    },
    withRouteDoc("测试 CIDR 库连接", {
      body: t.Object({
        url: t.String(),
      }),
    }),
  )
  .get(
    "/config/terminal_feature",
    async () => {
      const settings = await configManager.getTerminalFeatureConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取终端功能配置"),
  )
  .get(
    "/config/dashboard_display",
    async () => {
      const settings = await configManager.getDashboardDisplayConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取首页展示配置"),
  )
  .get(
    "/config/auto_https",
    async () => {
      const details = await buildAutoHttpsDetails();
      return { success: true, data: details };
    },
    routeDoc("获取自动 HTTPS 配置"),
  )
  .post(
    "/config/dashboard_display",
    async ({ body }) => {
      const next = await configManager.updateDashboardDisplayConfig(body);
      return { success: true, data: next };
    },
    withRouteDoc("更新首页展示配置", {
      body: t.Object({
        show_entry_status_module: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/config/auto_https",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const runtimeProfile = getRuntimeProfile();
      if (
        body.enabled === true &&
        (runtimeProfile.is_docker ||
          runtimeProfile.deployment_target === "openwrt")
      ) {
        set.status = 403;
        return {
          success: false,
          message:
            runtimeProfile.deployment_target === "openwrt"
              ? adminT(t, "autoHttps.openWrtUnsupported")
              : adminT(t, "autoHttps.dockerUnsupported"),
        };
      }

      if (body.enabled === true) {
        const runtime = await autoHttpsRedirectManager.applyConfig({
          enabled: true,
        });
        const next = await configManager.updateAutoHttpsConfig({
          enabled: runtime.status === "active",
        });
        return {
          success: true,
          data: {
            ...next,
            runtime,
          },
          message:
            runtime.status === "error"
              ? runtime.last_error || adminT(t, "autoHttps.startFailed")
              : undefined,
        };
      }

      const next = await configManager.updateAutoHttpsConfig(body);
      const runtime = await autoHttpsRedirectManager.applyConfig(next);
      return {
        success: true,
        data: {
          ...next,
          runtime,
        },
        message:
          runtime.status === "error"
            ? runtime.last_error || adminT(t, "autoHttps.startFailed")
            : undefined,
      };
    },
    withRouteDoc("更新自动 HTTPS 配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/config/terminal_feature",
    async ({ body }) => {
      const next = await configManager.updateTerminalFeatureConfig(body);
      return { success: true, data: next };
    },
    withRouteDoc("更新终端功能配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        default_cwd: t.Optional(t.String()),
        max_sessions: t.Optional(t.Number()),
        idle_timeout_seconds: t.Optional(t.Number()),
        resume_backend: t.Optional(t.Literal("tmux")),
        allow_mobile_toolbar: t.Optional(t.Boolean()),
        dangerously_run_as_current_user: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/default_route",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: { default_route: config.default_route } };
    },
    routeDoc("获取默认路由"),
  )
  .post(
    "/config/default_route",
    async ({ body }) => {
      await configManager.updateDefaultRoute(body.path);
      await goBackend.setDefaultRoute(body.path);
      return { success: true };
    },
    withRouteDoc("更新默认路由", {
      body: t.Object({
        path: t.String(),
      }),
    }),
  )
  .post(
    "/config/default_tunnel",
    async ({ body }) => {
      await configManager.updateDefaultTunnel(body.tunnel);
      return { success: true };
    },
    withRouteDoc("设置默认隧道类型", {
      body: t.Object({
        tunnel: t.Union([t.Literal("frp"), t.Literal("cloudflared")]),
      }),
    }),
  )
  .post(
    "/sync-routes",
    async ({ request, set }) => {
      const { t } = await getAdminRouteTranslator(request);
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
            message: adminT(t, "syncRoutes.partialFailedGatewayLogging", {
              gatewayLogging: loggingResult.success,
            }),
          };
        }

        let syncedWAF = true;
        try {
          await syncWAFConfigToGateway(config.waf ?? null);
        } catch (error) {
          syncedWAF = false;
          set.status = 502;
          return {
            success: false,
            message: adminT(t, "syncRoutes.partialFailedGatewayLoggingWaf", {
              gatewayLogging: loggingResult.success,
              waf: syncedWAF,
            }),
          };
        }

        const syncedRules =
          config.run_type === 1 && !isReverseProxySubdomainMode(config)
            ? config.proxy_mappings.length
            : 0;
        const syncedHostRules = isAnySubdomainRoutingMode(config)
          ? config.host_mappings.length
          : 0;
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
            synced_waf: syncedWAF,
            waf_bundle_id: config.waf?.active_bundle_id || "",
          },
          message: adminT(t, "syncRoutes.success", {
            hostRules: syncedHostRules,
            rules: syncedRules,
            streamRules: syncedStreamRules,
          }),
        };
      } catch (e: any) {
        set.status = 500;
        return { success: false, message: e?.message ?? String(e) };
      }
    },
    routeDoc("按当前配置同步路由与网关"),
  );
