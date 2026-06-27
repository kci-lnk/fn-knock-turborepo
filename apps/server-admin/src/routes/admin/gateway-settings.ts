import { Elysia, t } from "elysia";
import {
  DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
  DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
  DEFAULT_GATEWAY_PORTAL_CONFIG,
  DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
  DEFAULT_GATEWAY_VISIBILITY_CONFIG,
  DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  type AppConfig,
  configManager,
  type GatewayHostResponseRuntimeState,
  type GatewayProxyHeadersRuntimeState,
  type GatewayVisibilityRuntimeState,
} from "../../lib/redis";
import { goBackend } from "../../lib/go-backend";
import { createRequestTranslator } from "../../lib/i18n";
import {
  applyGatewayPortalIconHostRulesPatchIfNeeded,
  applyGatewayPortalTitleHostRulesPatchIfNeeded,
  syncGatewayPortalToGateway,
} from "../../lib/gateway-portal";
import { buildGatewayAuthConfig } from "../../lib/subdomain-mode";
import {
  buildGatewayHostResponseSummary,
  compileGatewayHostResponseState,
  getGatewayHostResponseDetails,
  syncGatewayHostResponseRuntimeForConfig,
  syncGatewayHostResponseToGateway,
} from "../../lib/gateway-host-response";
import {
  buildGatewayProxyHeadersSummary,
  compileGatewayProxyHeadersState,
  getGatewayProxyHeadersDetails,
  syncGatewayProxyHeadersToGateway,
} from "../../lib/gateway-proxy-headers";
import {
  buildGatewayVisibilitySummary,
  compileGatewayVisibilityConfig,
  getGatewayVisibilityDetails,
  syncGatewayVisibilityToGateway,
} from "../../lib/gateway-visibility";
import { syncGatewayCrawlerBlockerToGateway } from "../../lib/gateway-crawler-blocker";
import { syncReverseProxyTrustedIPsNow } from "../../lib/reverse-proxy-trusted-ips";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import {
  adminT,
  getAdminRouteTranslator,
  rollbackConfigAndRuntime,
  type RequestTranslator,
} from "./shared";

const rollbackGatewayVisibilityConfigAndRuntime = async (
  previousConfig: AppConfig,
  previousRuntime: GatewayVisibilityRuntimeState,
  t: RequestTranslator,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return (
      error?.message || adminT(t, "rollback.restoreVisibilityConfigFailed")
    );
  }

  try {
    await configManager.saveGatewayVisibilityRuntimeState(previousRuntime);
  } catch (error: any) {
    return (
      error?.message || adminT(t, "rollback.restoreVisibilityRuntimeFailed")
    );
  }

  try {
    await syncGatewayVisibilityToGateway(previousRuntime);
  } catch (error: any) {
    return (
      error?.message || adminT(t, "rollback.restoreGatewayVisibilityFailed")
    );
  }

  return null;
};

const rollbackGatewayProxyHeadersConfigAndRuntime = async (
  previousConfig: AppConfig,
  previousRuntime: GatewayProxyHeadersRuntimeState,
  t: RequestTranslator,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return (
      error?.message || adminT(t, "rollback.restoreProxyHeadersConfigFailed")
    );
  }

  try {
    await configManager.saveGatewayProxyHeadersRuntimeState(previousRuntime);
  } catch (error: any) {
    return (
      error?.message || adminT(t, "rollback.restoreProxyHeadersRuntimeFailed")
    );
  }

  try {
    await syncGatewayProxyHeadersToGateway(previousRuntime);
  } catch (error: any) {
    return (
      error?.message ||
      adminT(t, "rollback.restoreGatewayProxyHeadersRuntimeFailed")
    );
  }

  return null;
};

const rollbackGatewayHostResponseConfigAndRuntime = async (
  previousConfig: AppConfig,
  previousRuntime: GatewayHostResponseRuntimeState,
  t: RequestTranslator,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return (
      error?.message || t("server.gatewayHostResponse.restoreConfigFailed")
    );
  }

  try {
    await configManager.saveGatewayHostResponseRuntimeState(previousRuntime);
  } catch (error: any) {
    return (
      error?.message || t("server.gatewayHostResponse.restoreRuntimeFailed")
    );
  }

  try {
    await syncGatewayHostResponseToGateway(previousRuntime);
  } catch (error: any) {
    return (
      error?.message ||
      t("server.gatewayHostResponse.restoreGatewayRuntimeFailed")
    );
  }

  return null;
};

const buildGatewaySettingsResponse = (
  config: Pick<
    AppConfig,
    | "subdomain_mode"
    | "reverse_proxy_throttle"
    | "gateway_visibility"
    | "gateway_proxy_headers"
    | "gateway_host_response"
    | "gateway_crawler_blocker"
    | "gateway_portal"
    | "host_mappings"
  >,
  visibilityRuntime: GatewayVisibilityRuntimeState,
  proxyHeadersRuntime: GatewayProxyHeadersRuntimeState,
  hostResponseRuntime: GatewayHostResponseRuntimeState,
) => ({
  auth_cache_ttl_seconds: config.subdomain_mode?.auth_cache_ttl_seconds ?? 1,
  auth_cache_unauthorized_ttl_seconds:
    config.subdomain_mode?.auth_cache_unauthorized_ttl_seconds ?? 1,
  reverse_proxy_throttle: config.reverse_proxy_throttle ?? {
    ...DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  },
  visibility: buildGatewayVisibilitySummary(
    config.gateway_visibility ?? DEFAULT_GATEWAY_VISIBILITY_CONFIG,
    visibilityRuntime,
  ),
  proxy_headers: buildGatewayProxyHeadersSummary(
    compileGatewayProxyHeadersState(
      {
        run_type: 3,
        reverse_proxy_submode: "subdomain",
        host_mappings: config.host_mappings,
        gateway_proxy_headers:
          config.gateway_proxy_headers ?? DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
      },
      config.gateway_proxy_headers ?? DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
    ).items,
    proxyHeadersRuntime,
  ),
  host_response: buildGatewayHostResponseSummary(
    compileGatewayHostResponseState(
      {
        run_type: 3,
        reverse_proxy_submode: "subdomain",
        host_mappings: config.host_mappings,
        gateway_host_response:
          config.gateway_host_response ?? DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
      },
      config.gateway_host_response ?? DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
    ).items,
    hostResponseRuntime,
  ),
  crawler_blocker:
    config.gateway_crawler_blocker ?? DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
  portal: config.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
});

export const adminGatewaySettingsRoutes = new Elysia()
  .get(
    "/config/gateway",
    async () => {
      const [
        config,
        visibilityRuntime,
        proxyHeadersRuntime,
        hostResponseRuntime,
      ] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayVisibilityRuntimeState(),
        configManager.getGatewayProxyHeadersRuntimeState(),
        configManager.getGatewayHostResponseRuntimeState(),
      ]);
      return {
        success: true,
        data: buildGatewaySettingsResponse(
          config,
          visibilityRuntime,
          proxyHeadersRuntime,
          hostResponseRuntime,
        ),
      };
    },
    routeDoc("获取网关配置"),
  )
  .post(
    "/config/gateway",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
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

        if (body.portal) {
          await configManager.updateGatewayPortalConfig(body.portal);
        }

        if (body.crawler_blocker) {
          await configManager.updateGatewayCrawlerBlockerConfig(
            body.crawler_blocker,
          );
        }

        const updatedConfig = await configManager.getConfig();
        const [
          visibilityRuntime,
          proxyHeadersRuntime,
          hostResponseRuntime,
          authConfigResult,
          reverseProxyThrottleResult,
          crawlerBlockerResult,
        ] = await Promise.all([
          configManager.getGatewayVisibilityRuntimeState(),
          configManager.getGatewayProxyHeadersRuntimeState(),
          configManager.getGatewayHostResponseRuntimeState(),
          goBackend.setAuthConfig(buildGatewayAuthConfig(updatedConfig)),
          goBackend.setReverseProxyThrottle(
            updatedConfig.reverse_proxy_throttle ??
              DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
          ),
          syncGatewayCrawlerBlockerToGateway(
            updatedConfig.gateway_crawler_blocker ??
              DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
          ).then(
            (data) => ({ success: true as const, data }),
            (error) => ({
              success: false as const,
              message:
                error?.message || adminT(t, "gateway.syncCrawlerBlockerFailed"),
            }),
          ),
        ]);

        const syncErrors: string[] = [];
        if (!authConfigResult.success) {
          syncErrors.push(
            authConfigResult.message ||
              adminT(t, "gateway.syncAuthCacheFailed"),
          );
        }
        if (!reverseProxyThrottleResult.success) {
          syncErrors.push(
            reverseProxyThrottleResult.message ||
              adminT(t, "gateway.syncThrottleFailed"),
          );
        }
        if (!crawlerBlockerResult.success) {
          syncErrors.push(
            crawlerBlockerResult.message ||
              adminT(t, "gateway.syncCrawlerBlockerFailed"),
          );
        }
        if (syncErrors.length > 0) {
          throw new Error(syncErrors.join("; "));
        }

        try {
          await syncReverseProxyTrustedIPsNow({
            config: updatedConfig,
          });
        } catch (error) {
          console.error(
            "[reverse-proxy-trusted-ips] failed to sync after gateway config update:",
            error,
          );
          throw error;
        }

        await syncGatewayPortalToGateway(
          updatedConfig.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
        );
        await applyGatewayPortalTitleHostRulesPatchIfNeeded(updatedConfig);
        await applyGatewayPortalIconHostRulesPatchIfNeeded(updatedConfig);

        return {
          success: true,
          data: buildGatewaySettingsResponse(
            updatedConfig,
            visibilityRuntime,
            proxyHeadersRuntime,
            hostResponseRuntime,
          ),
        };
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(previousConfig, t);
        let portalRollbackError: string | null = null;
        try {
          await syncGatewayPortalToGateway(
            previousConfig.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
          );
        } catch (innerError: any) {
          portalRollbackError =
            innerError?.message || adminT(t, "rollback.restorePortalFailed");
        }
        set.status = 502;
        const extraRollbackError = [rollbackError, portalRollbackError]
          .filter(Boolean)
          .join("；");
        return {
          success: false,
          message: extraRollbackError
            ? adminT(t, "rollback.failed", {
                message: error?.message || adminT(t, "gateway.updateFailed"),
                rollbackError: extraRollbackError,
              })
            : error?.message || adminT(t, "gateway.updateFailedRolledBack"),
        };
      }
    },
    withRouteDoc("更新网关配置", {
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
        portal: t.Optional(
          t.Object({
            enabled: t.Optional(t.Boolean()),
            display_style: t.Optional(
              t.Union([t.Literal("domain"), t.Literal("title")]),
            ),
            show_app_icon: t.Optional(t.Boolean()),
          }),
        ),
        crawler_blocker: t.Optional(
          t.Object({
            enabled: t.Optional(t.Boolean()),
          }),
        ),
      }),
    }),
  )
  .get(
    "/config/gateway/visibility",
    async () => {
      const details = await getGatewayVisibilityDetails();
      return {
        success: true,
        data: details,
      };
    },
    routeDoc("获取网关可见性配置"),
  )
  .post(
    "/config/gateway/visibility",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const [previousConfig, previousRuntime] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayVisibilityRuntimeState(),
      ]);

      try {
        const compiled = await compileGatewayVisibilityConfig({
          enabled: body.enabled,
          selections: body.selections,
          custom_cidrs: body.custom_cidrs,
        });

        const [savedConfig, savedRuntime] = await Promise.all([
          configManager.updateGatewayVisibilityConfig(compiled.config),
          configManager.saveGatewayVisibilityRuntimeState(compiled.runtime),
        ]);

        await syncGatewayVisibilityToGateway(savedRuntime);

        return {
          success: true,
          data: {
            config: savedConfig,
            summary: buildGatewayVisibilitySummary(savedConfig, savedRuntime),
          },
        };
      } catch (error: any) {
        const rollbackError = await rollbackGatewayVisibilityConfigAndRuntime(
          previousConfig,
          previousRuntime,
          t,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message || adminT(t, "gatewayVisibility.updateFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "gatewayVisibility.updateFailedRolledBack"),
        };
      }
    },
    withRouteDoc("更新网关可见性配置", {
      body: t.Object({
        enabled: t.Boolean(),
        selections: t.Array(
          t.Object({
            province: t.String(),
            query_city: t.Optional(t.Union([t.String(), t.Null()])),
          }),
        ),
        custom_cidrs: t.Array(t.String()),
      }),
    }),
  )
  .get(
    "/config/gateway/proxy-headers",
    async () => {
      const details = await getGatewayProxyHeadersDetails();
      return {
        success: true,
        data: details,
      };
    },
    routeDoc("获取网关代理请求头配置"),
  )
  .post(
    "/config/gateway/proxy-headers",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const [previousConfig, previousRuntime] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayProxyHeadersRuntimeState(),
      ]);

      if (!isAnySubdomainRoutingMode(previousConfig)) {
        set.status = 400;
        return {
          success: false,
          message: adminT(t, "gatewayProxyHeaders.subdomainOnly"),
        };
      }

      try {
        const compiled = compileGatewayProxyHeadersState(previousConfig, {
          disabled_hosts: body.disabled_hosts,
        });

        await Promise.all([
          configManager.updateGatewayProxyHeadersConfig(compiled.config),
          configManager.saveGatewayProxyHeadersRuntimeState(compiled.runtime),
        ]);

        await syncGatewayProxyHeadersToGateway(compiled.runtime);

        return {
          success: true,
          data: await getGatewayProxyHeadersDetails(),
        };
      } catch (error: any) {
        const rollbackError = await rollbackGatewayProxyHeadersConfigAndRuntime(
          previousConfig,
          previousRuntime,
          t,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message ||
                  adminT(t, "gatewayProxyHeaders.updateFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "gatewayProxyHeaders.updateFailedRolledBack"),
        };
      }
    },
    withRouteDoc("更新网关代理请求头配置", {
      body: t.Object({
        disabled_hosts: t.Array(t.String()),
      }),
    }),
  )
  .get(
    "/config/gateway/host-response",
    async ({ request }) => {
      const { locale } = await getAdminRouteTranslator(request);
      const details = await getGatewayHostResponseDetails(locale);
      return {
        success: true,
        data: details,
      };
    },
    routeDoc("获取网关 Host 响应配置"),
  )
  .post(
    "/config/gateway/host-response",
    async ({ body, set, request }) => {
      const [previousConfig, previousRuntime] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayHostResponseRuntimeState(),
      ]);
      const { locale, t } = createRequestTranslator(
        request,
        previousConfig.locale,
      );

      if (!isAnySubdomainRoutingMode(previousConfig)) {
        set.status = 400;
        return {
          success: false,
          message: t("server.gatewayHostResponse.editSubdomainOnly"),
        };
      }

      try {
        await syncGatewayHostResponseRuntimeForConfig(
          {
            run_type: previousConfig.run_type,
            reverse_proxy_submode: previousConfig.reverse_proxy_submode,
            host_mappings: previousConfig.host_mappings,
            gateway_host_response: {
              disabled_hosts: body.disabled_hosts,
            },
          },
          {
            saveConfig: true,
          },
        );

        return {
          success: true,
          data: await getGatewayHostResponseDetails(locale),
        };
      } catch (error: any) {
        const rollbackError = await rollbackGatewayHostResponseConfigAndRuntime(
          previousConfig,
          previousRuntime,
          t,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? t("server.gatewayHostResponse.updateFailedRollbackFailed", {
                error:
                  error?.message ||
                  t("server.gatewayHostResponse.updateFailed"),
                rollbackError,
              })
            : error?.message ||
              t("server.gatewayHostResponse.updateFailedRolledBack"),
        };
      }
    },
    withRouteDoc("更新网关 Host 响应配置", {
      body: t.Object({
        disabled_hosts: t.Array(t.String()),
      }),
    }),
  );
