import { Elysia, t } from "elysia";
import {
  type AppConfig,
  configManager,
  type HostMapping,
  type ProxyMapping,
} from "../../lib/redis";
import { goBackend } from "../../lib/go-backend";
import { firewallService } from "../../lib/firewall-service";
import { isAuthServiceTarget } from "../../lib/auth-service";
import {
  resolveHostMappingDisplayTitle,
  refreshAllHostMappingTitles,
  scheduleHostMappingsMetadataRefresh,
} from "../../lib/host-mapping-metadata";
import {
  buildGatewayAuthConfig,
  buildSubdomainCertificateInventoryCoverage,
  getAuthHostMapping,
  resolvePublicPortForScheme,
} from "../../lib/subdomain-mode";
import { fetchUrlMetadata } from "../../lib/url-metadata";
import { probeBasicAuthTarget } from "../../lib/basic-auth-probe";
import {
  buildHostMappingsBookmarkFilename,
  buildHostMappingsBookmarksDocument,
} from "../../lib/host-mapping-bookmarks";
import { syncGatewayPortalHostRulesIfTitleMode } from "../../lib/gateway-portal";
import { syncGatewayHostResponseRuntimeForConfig } from "../../lib/gateway-host-response";
import { syncGatewayProxyHeadersRuntimeForConfig } from "../../lib/gateway-proxy-headers";
import { scheduleSmartConnectSyncAfterHostMappingsChange } from "../../lib/smart-connect";
import { syncSSLDeploymentToGateway } from "../../lib/ssl-gateway";
import {
  resolveAccessEntryInfo,
  shouldOmitPublicAccessEntryPort,
} from "../../lib/access-entry";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import {
  createDisabledHostBasicAuth,
  normalizeHostBasicAuth,
  normalizeHostMappingLocationsForRoute,
  normalizeHostMappingLookupKey,
  validateHostMappings,
  validateProxyMappings,
  validateStreamMappings,
} from "./validation";
import {
  adminT,
  ensureGoResponseSuccess,
  getAdminRouteTranslator,
  isSameJsonValue,
  normalizeHostLike,
  rollbackConfigAndRuntime,
  type RequestTranslator,
} from "./shared";

const toHostRuleSyncPayload = (
  mapping: Pick<
    HostMapping,
    | "host"
    | "target"
    | "use_auth"
    | "access_mode"
    | "suppress_toolbar"
    | "preserve_host"
    | "is_default"
    | "basic_auth"
    | "locations"
    | "title"
    | "title_override"
    | "favicon"
  >,
) => ({
  host: normalizeHostMappingLookupKey(mapping.host),
  target: mapping.target.trim(),
  use_auth: mapping.use_auth,
  access_mode: mapping.access_mode,
  suppress_toolbar: mapping.suppress_toolbar,
  preserve_host: mapping.preserve_host,
  is_default: mapping.is_default === true,
  title: resolveHostMappingDisplayTitle(mapping),
  favicon:
    typeof mapping.favicon === "string" && mapping.favicon.trim()
      ? mapping.favicon.trim()
      : null,
  basic_auth: normalizeHostBasicAuth(mapping.basic_auth),
  locations: mapping.locations ?? [],
});

const haveSyncedHostRulesChanged = (
  previousMappings: HostMapping[],
  nextMappings: HostMapping[],
): boolean =>
  JSON.stringify(previousMappings.map(toHostRuleSyncPayload)) !==
  JSON.stringify(nextMappings.map(toHostRuleSyncPayload));

const resolveBookmarkScheme = (
  config: Pick<AppConfig, "ssl">,
): "http" | "https" =>
  config.ssl.cert.trim() && config.ssl.key.trim() ? "https" : "http";

const validatePasskeyRpConfig = (
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
  t: RequestTranslator,
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
      message: adminT(t, "passkeyRp.parentDomainRequired"),
    };
  }

  const authHost = normalizeHostLike(
    getAuthHostMapping(config)?.host || config.subdomain_mode?.auth_host,
  );
  if (authHost && authHost !== rpId && !authHost.endsWith(`.${rpId}`)) {
    return {
      valid: false as const,
      message: adminT(t, "passkeyRp.mustMatchAuthHost", { authHost, rpId }),
    };
  }

  return { valid: true as const };
};

const rollbackProxyMappingsConfigAndRuntime = async (
  previousConfig: AppConfig,
  t: RequestTranslator,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreConfigFailed");
  }

  try {
    ensureGoResponseSuccess(
      await goBackend.setRules(previousConfig.proxy_mappings),
      adminT(t, "proxyMappings.restoreRulesFailed"),
    );
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreRuntimeFailed");
  }

  return null;
};

const syncHostMappingsRuntime = async (
  previousConfig: AppConfig,
  nextConfig: AppConfig,
  normalizedMappings: HostMapping[],
  t: RequestTranslator,
): Promise<void> => {
  const previousGatewayAuthConfig = buildGatewayAuthConfig(previousConfig);
  const nextGatewayAuthConfig = buildGatewayAuthConfig(nextConfig);

  if (
    haveSyncedHostRulesChanged(previousConfig.host_mappings, normalizedMappings)
  ) {
    ensureGoResponseSuccess(
      await goBackend.setHostRules(normalizedMappings),
      adminT(t, "hostMappings.syncHostRulesFailed"),
    );
  }

  if (!isSameJsonValue(previousGatewayAuthConfig, nextGatewayAuthConfig)) {
    ensureGoResponseSuccess(
      await goBackend.setAuthConfig(nextGatewayAuthConfig),
      adminT(t, "hostMappings.syncAuthConfigFailed"),
    );
  }

  await syncGatewayProxyHeadersRuntimeForConfig(nextConfig, {
    saveConfig: true,
  });
  await syncGatewayHostResponseRuntimeForConfig(nextConfig, {
    saveConfig: true,
  });
};

export const adminProxyMappingsRoutes = new Elysia()
  .post(
    "/config/proxy_mappings",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const validation = validateProxyMappings(body.mappings, t);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const config = await configManager.getConfig();
      const normalizedMappings: ProxyMapping[] = body.mappings.map(
        (mapping) => ({
          ...mapping,
          target: mapping.target.trim(),
        }),
      );
      const updatedConfig = {
        ...config,
        proxy_mappings: normalizedMappings,
      };

      try {
        await configManager.saveConfig(updatedConfig);
        ensureGoResponseSuccess(
          await goBackend.setRules(normalizedMappings),
          adminT(t, "proxyMappings.syncRulesFailed"),
        );
      } catch (error: any) {
        const rollbackError = await rollbackProxyMappingsConfigAndRuntime(
          config,
          t,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message || adminT(t, "proxyMappings.updateFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "proxyMappings.updateFailedRolledBack"),
        };
      }

      return { success: true, data: normalizedMappings };
    },
    withRouteDoc("更新路径代理映射", {
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
    }),
  )
  .get(
    "/config/host_mappings",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: config.host_mappings };
    },
    routeDoc("获取 Host 映射列表"),
  )
  .post(
    "/config/host_mappings",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const validation = validateHostMappings(body.mappings, t);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const config = await configManager.getConfig();
      const previousByHost = new Map(
        config.host_mappings.map((mapping) => [
          normalizeHostMappingLookupKey(mapping.host),
          mapping,
        ]),
      );
      let hasDefaultMapping = false;
      const normalizedMappings: HostMapping[] = body.mappings.map((mapping) => {
        const previous = previousByHost.get(
          normalizeHostMappingLookupKey(mapping.host),
        );
        const normalizedTarget = mapping.target.trim();
        const serviceRole = isAuthServiceTarget(normalizedTarget)
          ? "auth"
          : "app";
        const canReusePreviousMetadata =
          previous?.target.trim() === normalizedTarget;
        const normalizedBasicAuth =
          serviceRole === "auth"
            ? createDisabledHostBasicAuth()
            : normalizeHostBasicAuth(
                mapping.basic_auth ?? previous?.basic_auth,
              );
        const normalizedLocations =
          serviceRole === "auth"
            ? []
            : normalizeHostMappingLocationsForRoute(mapping.locations);
        const isDefault =
          serviceRole !== "auth" &&
          mapping.is_default === true &&
          !hasDefaultMapping;
        if (isDefault) {
          hasDefaultMapping = true;
        }

        return {
          ...mapping,
          target: normalizedTarget,
          service_role: serviceRole,
          is_default: isDefault,
          basic_auth: normalizedBasicAuth,
          locations: normalizedLocations,
          title:
            typeof mapping.title === "string"
              ? mapping.title.trim()
              : canReusePreviousMetadata
                ? (previous?.title ?? "")
                : "",
          title_override:
            typeof mapping.title_override === "string"
              ? mapping.title_override.trim()
              : (previous?.title_override ?? ""),
          favicon:
            typeof mapping.favicon === "string"
              ? mapping.favicon.trim()
              : canReusePreviousMetadata
                ? (previous?.favicon ?? "")
                : "",
        };
      });
      const nextConfig = {
        ...config,
        host_mappings: normalizedMappings,
      };
      const passkeyValidation = validatePasskeyRpConfig(nextConfig, t);
      if (!passkeyValidation.valid) {
        set.status = 400;
        return {
          success: false,
          message: passkeyValidation.message,
        };
      }

      const updatedConfig = {
        ...config,
        host_mappings: normalizedMappings,
      };

      try {
        await configManager.saveConfig(updatedConfig);
        await syncHostMappingsRuntime(
          config,
          updatedConfig,
          normalizedMappings,
          t,
        );
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(config, t);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message || adminT(t, "hostMappings.updateFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "hostMappings.updateFailedRolledBack"),
        };
      }

      scheduleHostMappingsMetadataRefresh(
        normalizedMappings,
        config.host_mappings,
      );
      scheduleSmartConnectSyncAfterHostMappingsChange(updatedConfig);

      return { success: true, data: normalizedMappings };
    },
    withRouteDoc("更新 Host 映射列表", {
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
            is_default: t.Optional(t.Boolean()),
            basic_auth: t.Optional(
              t.Object({
                enabled: t.Boolean(),
                username: t.String(),
                password: t.String(),
              }),
            ),
            locations: t.Optional(
              t.Array(
                t.Object({
                  path: t.String(),
                  match: t.Union([t.Literal("exact"), t.Literal("prefix")]),
                  action: t.Union([t.Literal("proxy"), t.Literal("response")]),
                  target: t.Optional(t.String()),
                  strip_path: t.Optional(t.Boolean()),
                  rewrite_html: t.Optional(t.Boolean()),
                  response: t.Optional(
                    t.Object({
                      status: t.Optional(t.Number()),
                      content_type: t.Optional(t.String()),
                      headers: t.Optional(t.Record(t.String(), t.String())),
                      body: t.Optional(t.String()),
                    }),
                  ),
                }),
              ),
            ),
            service_role: t.Optional(
              t.Union([t.Literal("app"), t.Literal("auth")]),
            ),
            title: t.Optional(t.String()),
            title_override: t.Optional(t.String()),
            favicon: t.Optional(t.String()),
          }),
        ),
      }),
    }),
  )
  .post(
    "/config/host_mappings/basic_auth_probe",
    async ({ body }) => {
      const result = await probeBasicAuthTarget(body.target);
      return {
        success: true,
        data: result,
      };
    },
    withRouteDoc("探测目标 Basic Auth", {
      body: t.Object({
        target: t.String(),
      }),
    }),
  )
  .post(
    "/config/host_mappings/metadata",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const metadata = await fetchUrlMetadata(body.target, {
        basicAuth: normalizeHostBasicAuth(body.basic_auth),
      });
      if (!metadata.ok) {
        set.status = 400;
        return {
          success: false,
          message: metadata.error || adminT(t, "hostMappings.metadataFailed"),
        };
      }

      return {
        success: true,
        data: metadata.data,
      };
    },
    withRouteDoc("抓取目标地址元数据", {
      body: t.Object({
        target: t.String(),
        basic_auth: t.Optional(
          t.Object({
            enabled: t.Boolean(),
            username: t.String(),
            password: t.String(),
          }),
        ),
      }),
    }),
  )
  .post(
    "/config/host_mappings/refresh_titles",
    async () => {
      const config = await configManager.getConfig();
      const { mappings, summary } = await refreshAllHostMappingTitles(
        config.host_mappings,
      );

      await configManager.updateHostMappings(mappings);
      await syncGatewayPortalHostRulesIfTitleMode({
        run_type: config.run_type,
        reverse_proxy_submode: config.reverse_proxy_submode,
        gateway_portal: config.gateway_portal,
        host_mappings: mappings,
      });

      return {
        success: true,
        data: summary,
      };
    },
    routeDoc("批量刷新 Host 映射标题"),
  )
  .get(
    "/config/host_mappings/bookmarks/export",
    async ({ request }) => {
      const { t } = await getAdminRouteTranslator(request);
      const config = await configManager.getConfig();
      const scheme = resolveBookmarkScheme(config);
      const accessEntryPort =
        resolvePublicPortForScheme(
          config,
          scheme,
          config.subdomain_mode?.public_auth_base_url || "",
        ) ?? null;
      const document = buildHostMappingsBookmarksDocument({
        mappings: config.host_mappings,
        scheme,
        accessEntryPort: accessEntryPort ?? resolveAccessEntryInfo(config).port,
        omitAccessEntryPort:
          shouldOmitPublicAccessEntryPort(config) && accessEntryPort === null,
        folderTitle: config.subdomain_mode?.root_domain?.trim()
          ? adminT(t, "hostMappings.bookmarkFolderForRoot", {
              root: config.subdomain_mode.root_domain.trim(),
            })
          : adminT(t, "hostMappings.bookmarkFolderDefault"),
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
    },
    routeDoc("导出 Host 映射书签"),
  )
  .get(
    "/config/stream_mappings",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: config.stream_mappings };
    },
    routeDoc("获取协议映射列表"),
  )
  .post(
    "/config/stream_mappings",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const validation = validateStreamMappings(body.mappings, t);
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
        const rollbackError = await rollbackConfigAndRuntime(previousConfig, t);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? adminT(t, "rollback.failed", {
                message:
                  error?.message || adminT(t, "streamMappings.syncFailed"),
                rollbackError,
              })
            : error?.message ||
              adminT(t, "streamMappings.syncFailedRolledBack"),
        };
      }

      return { success: true };
    },
    withRouteDoc("更新协议映射列表", {
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
    }),
  )
  .get(
    "/config/subdomain_mode",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: config.subdomain_mode };
    },
    routeDoc("获取子域模式配置"),
  )
  .post(
    "/config/subdomain_mode",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const config = await configManager.getConfig();
      const nextConfig = {
        ...config,
        subdomain_mode: {
          ...config.subdomain_mode,
          ...body,
        },
      };
      const validation = validateHostMappings(nextConfig.host_mappings, t);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const passkeyValidation = validatePasskeyRpConfig(nextConfig, t);
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
        t,
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
              message: adminT(t, "subdomainMode.sslAutoSelected"),
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
                adminT(t, "subdomainMode.sslAutoSelectionSyncFailed"),
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
    withRouteDoc("更新子域模式配置", {
      body: t.Object({
        root_domain: t.Optional(t.String()),
        auth_host: t.Optional(t.String()),
        auth_target: t.Optional(t.String()),
        cookie_domain: t.Optional(t.String()),
        edge_client_ip_enabled: t.Optional(t.Boolean()),
        aliyun_esa_enabled: t.Optional(t.Boolean()),
        tencent_edgeone_enabled: t.Optional(t.Boolean()),
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
    }),
  );
