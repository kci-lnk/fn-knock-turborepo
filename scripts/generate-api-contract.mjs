#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const mode = process.argv[2] ?? "check";
if (!new Set(["generate", "check"]).has(mode)) {
  throw new Error("usage: generate-api-contract.mjs [generate|check]");
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    throw new Error(
      [`${command} ${args.join(" ")} failed`, result.stdout, result.stderr]
        .filter(Boolean)
        .join("\n"),
    );
  }
  if (result.stdout.trim()) console.log(result.stdout.trim());
}

function validateContract(openapiPath) {
  const document = JSON.parse(readFileSync(openapiPath, "utf8"));
  if (document.openapi !== "3.1.0") {
    throw new Error(`expected OpenAPI 3.1.0, got ${document.openapi}`);
  }
  const typedDomainOperations = new Map([
    ["post /api/internal/system-events", "SystemEventPublishBodyData"],
    ["get /api/admin/events", null],
    ["delete /api/admin/events", "SystemEventDeleteBodyData"],
    ["delete /api/admin/events/clear", null],
    ["get /api/admin/backoff/list", null],
    ["get /api/admin/backoff/status", null],
    ["post /api/admin/backoff/reset", "LoginBackoffResetBodyData"],
    ["get /api/admin/config/captcha", null],
    ["post /api/admin/config/captcha", "CaptchaSettingsUpdateData"],
    ["post /api/admin/config/run_type", "RunTypeUpdateData"],
    [
      "post /api/admin/config/auto_manage_firewall",
      "AutoManageFirewallUpdateData",
    ],
    ["get /api/admin/config/terminal_feature", null],
    ["post /api/admin/config/terminal_feature", "TerminalFeatureUpdateData"],
    ["get /api/admin/config/welcome_guide", null],
    ["post /api/admin/config/welcome_guide/complete", null],
    ["get /api/admin/config/appearance", null],
    ["post /api/admin/config/appearance", "PanelAppearanceData"],
    ["get /api/admin/config/auto_https", null],
    ["post /api/admin/config/auto_https", "AutoHttpsUpdateData"],
    ["get /api/admin/config/default_route", null],
    ["post /api/admin/config/default_route", "DefaultRouteUpdateData"],
    ["post /api/admin/config/default_tunnel", "DefaultTunnelUpdateData"],
    ["get /api/admin/config/firewall_additional_ports", null],
    [
      "post /api/admin/config/firewall_additional_ports",
      "FirewallAdditionalPortsUpdateData",
    ],
    ["post /api/admin/firewall/reset", "FirewallResetBodyData"],
    ["post /api/admin/firewall/clear", null],
    ["post /api/admin/sync-routes", null],
    ["post /api/admin/maintenance/data/clear", "MaintenanceClearBodyData"],
    ["get /api/admin/system/access-entry", null],
    ["get /api/admin/system/clock/status", null],
    ["post /api/admin/system/clock/check", null],
    ["post /api/admin/system/clock/sync", null],
    ["get /api/admin/system/cloudflared/status", null],
    ["post /api/admin/system/cloudflared/download", null],
    ["post /api/admin/system/cloudflared/cancel", null],
    ["delete /api/admin/system/cloudflared", null],
    ["get /api/admin/system/frp/status", null],
    ["post /api/admin/system/frp/download", null],
    ["post /api/admin/system/frp/cancel", null],
    ["delete /api/admin/system/frp", null],
    ["get /api/admin/system/dnsmasq/status", null],
    ["post /api/admin/system/dnsmasq/install", null],
    ["get /api/admin/terminal/status", null],
    ["post /api/admin/terminal/tmux/install", null],
    ["get /api/admin/terminal/sessions", null],
    ["post /api/admin/terminal/sessions", "TerminalCreateSessionBodyData"],
    ["get /api/admin/terminal/sessions/{id}", null],
    [
      "patch /api/admin/terminal/sessions/{id}",
      "TerminalRenameSessionBodyData",
    ],
    ["delete /api/admin/terminal/sessions/{id}", null],
    ["post /api/admin/terminal/sessions/{id}/attachments", null],
    ["get /api/admin/terminal/attachments/{id}/poll", null],
    [
      "post /api/admin/terminal/attachments/{id}/input",
      "TerminalInputBodyData",
    ],
    [
      "post /api/admin/terminal/attachments/{id}/resize",
      "TerminalResizeBodyData",
    ],
    ["delete /api/admin/terminal/attachments/{id}", null],
    ["get /api/admin/cloudflared/status", null],
    ["get /api/admin/cloudflared/config", null],
    ["post /api/admin/cloudflared/config", "CloudflaredConfigUpdateData"],
    ["post /api/admin/cloudflared/start", null],
    ["post /api/admin/cloudflared/stop", null],
    ["get /api/admin/cloudflared/logs", null],
    ["delete /api/admin/cloudflared/logs", null],
    ["get /api/admin/cloudflared/poll", null],
    [
      "put /api/admin/cloudflared/cloudflare/credential",
      "CloudflareCredentialBodyData",
    ],
    ["delete /api/admin/cloudflared/cloudflare/credential", null],
    ["get /api/admin/cloudflared/cloudflare/state", null],
    [
      "post /api/admin/cloudflared/reconcile/preview",
      "CloudflareReconcileRequestData",
    ],
    [
      "post /api/admin/cloudflared/reconcile/apply",
      "CloudflareReconcileApplyBodyData",
    ],
    ["get /api/admin/cloudflared/reconcile/jobs/active", null],
    ["get /api/admin/cloudflared/reconcile/jobs/{id}", null],
    ["get /api/admin/cloudflared/reconcile/jobs/by-plan/{plan_id}", null],
    [
      "post /api/admin/cloudflared/optimization/scans",
      "CloudflareOptimizationScanBodyData",
    ],
    ["get /api/admin/cloudflared/optimization/scans/{id}", null],
    ["delete /api/admin/cloudflared/optimization/scans/{id}", null],
    [
      "post /api/admin/cloudflared/optimization/apply",
      "CloudflareOptimizationApplyBodyData",
    ],
    ["post /api/admin/cloudflared/optimization/fallback", null],
    [
      "put /api/admin/cloudflared/optimization/settings",
      "CloudflareOptimizationSourceSettingsBodyData",
    ],
    [
      "put /api/admin/cloudflared/optimization/domains/{hostname}",
      "CloudflareOptimizationDomainBodyData",
    ],
    ["get /api/admin/frpc/status", null],
    ["get /api/admin/frpc/overview", null],
    ["get /api/admin/frpc/web-status", null],
    ["get /api/admin/frpc/config", null],
    ["post /api/admin/frpc/config", "FrpcConfigUpdateData"],
    ["post /api/admin/frpc/start", null],
    ["post /api/admin/frpc/stop", null],
    ["get /api/admin/frpc/logs", null],
    ["delete /api/admin/frpc/logs", null],
    ["get /api/admin/frpc/poll", null],
    ["get /api/admin/frpc/instances", null],
    ["post /api/admin/frpc/instances", "FrpcInstanceBodyData"],
    ["post /api/admin/frpc/instances/draft", null],
    ["get /api/admin/frpc/instances/{id}", null],
    ["put /api/admin/frpc/instances/{id}", "FrpcInstanceBodyData"],
    ["delete /api/admin/frpc/instances/{id}", null],
    ["post /api/admin/frpc/instances/{id}/start", null],
    ["post /api/admin/frpc/instances/{id}/stop", null],
    ["post /api/admin/frpc/instances/{id}/restart", null],
    ["get /api/admin/frpc/instances/{id}/logs", null],
    ["delete /api/admin/frpc/instances/{id}/logs", null],
    ["get /api/admin/frpc/instances/{id}/poll", null],
    ["get /api/admin/ddns/status", null],
    ["post /api/admin/ddns/toggle", "DdnsToggleBodyData"],
    ["get /api/admin/ddns/providers", null],
    ["get /api/admin/ddns/settings", null],
    ["post /api/admin/ddns/settings", "DdnsSettingsUpdateData"],
    ["post /api/admin/ddns/public-check/test", "DdnsPublicCheckTestBodyData"],
    ["get /api/admin/ddns/interfaces", null],
    [
      "post /api/admin/ddns/interfaces/resolve",
      "DdnsInterfaceSelectorPreviewBodyData",
    ],
    ["post /api/admin/ddns/provider", "DdnsProviderBodyData"],
    ["get /api/admin/ddns/config/{provider}", null],
    ["post /api/admin/ddns/config/{provider}", "DdnsConfigBodyData"],
    ["get /api/admin/ddns/targets", null],
    ["post /api/admin/ddns/targets", "DdnsTargetBodyData"],
    ["get /api/admin/ddns/targets/{id}", null],
    ["put /api/admin/ddns/targets/{id}", "DdnsTargetBodyData"],
    ["delete /api/admin/ddns/targets/{id}", null],
    ["post /api/admin/ddns/targets/{id}/enabled", "DdnsTargetEnabledBodyData"],
    ["post /api/admin/ddns/test", null],
    ["post /api/admin/ddns/targets/{id}/test", null],
    ["get /api/admin/ddns/logs", null],
    ["delete /api/admin/ddns/logs", null],
    ["get /api/admin/ddns/poll", null],
    ["get /api/admin/ssl/status", null],
    ["get /api/admin/ssl/shared-files", null],
    ["get /api/admin/ssl/shared-files/content", null],
    ["get /api/admin/ssl/cert.pem", null],
    ["get /api/admin/ssl/cert.zip", null],
    ["get /api/admin/ssl/ca/status", null],
    ["post /api/admin/ssl/ca/init", null],
    ["delete /api/admin/ssl/ca", null],
    ["get /api/admin/ssl/ca/cert.pem", null],
    ["get /api/admin/ssl/ca/server-cert.zip", null],
    ["get /api/admin/ssl/ca/hosts", null],
    ["post /api/admin/ssl/ca/hosts", "SslCaHostBodyData"],
    ["delete /api/admin/ssl/ca/hosts", "SslCaHostsDeleteBodyData"],
    ["post /api/admin/ssl/ca/issue", null],
    ["post /api/admin/ssl/certificates", "SslCertificateSaveBodyData"],
    ["delete /api/admin/ssl/certificates", null],
    ["delete /api/admin/ssl/certificates/{id}", null],
    ["post /api/admin/ssl/activate", "SslCertificateActivateBodyData"],
    ["post /api/admin/ssl/deployment-mode", "SslDeploymentModeBodyData"],
    ["delete /api/admin/ssl", null],
    ["get /api/admin/waf/details", null],
    ["get /api/admin/waf/status", null],
    ["post /api/admin/waf/config", "WafConfigUpdateData"],
    ["post /api/admin/waf/manifest/refresh", null],
    ["post /api/admin/waf/system/sync", null],
    ["post /api/admin/waf/rules/recommended", null],
    ["post /api/admin/waf/rules/enabled", "WafRuleToggleBodyData"],
    ["get /api/admin/waf/rules/{source}/{filename}", null],
    ["post /api/admin/waf/custom/upload", "WafUploadBodyData"],
    ["delete /api/admin/waf/custom/{filename}", null],
    ["post /api/admin/waf/events/drain", null],
    ["get /api/admin/waf/logs", null],
    ["get /api/admin/waf/logs/{trace_id}", null],
    ["delete /api/admin/waf/logs", "WafLogDeleteBodyData"],
    ["get /api/admin/notifications/providers/catalog", null],
    ["get /api/admin/notifications/providers", null],
    [
      "post /api/admin/notifications/providers",
      "NotificationProviderCreateBodyData",
    ],
    [
      "post /api/admin/notifications/providers/test",
      "NotificationProviderTestBodyData",
    ],
    ["get /api/admin/notifications/providers/{id}", null],
    [
      "patch /api/admin/notifications/providers/{id}",
      "NotificationProviderUpdateBodyData",
    ],
    ["delete /api/admin/notifications/providers/{id}", null],
    ["post /api/admin/notifications/providers/{id}/test", null],
    ["get /api/admin/notifications/rules", null],
    ["post /api/admin/notifications/rules", "NotificationRuleCreateBodyData"],
    [
      "patch /api/admin/notifications/rules/{id}",
      "NotificationRuleUpdateBodyData",
    ],
    ["delete /api/admin/notifications/rules/{id}", null],
    ["get /api/admin/notifications/triggers", null],
    ["get /api/admin/notifications/deliveries", null],
    [
      "delete /api/admin/notifications/deliveries",
      "NotificationDeliveryClearBodyData",
    ],
    ["get /api/admin/config/protocol_mapping_feature", null],
    [
      "post /api/admin/config/protocol_mapping_feature",
      "ProtocolMappingFeatureUpdateData",
    ],
    ["get /api/admin/config/proxy_protocol_force", null],
    ["post /api/admin/config/proxy_protocol_force", "ProxyProtocolForceData"],
    ["get /api/admin/config/run_mode_prompt_preferences", null],
    [
      "post /api/admin/config/run_mode_prompt_preferences",
      "RunModePromptPreferencesUpdateData",
    ],
    ["get /api/admin/config/smart_connect/details", null],
    ["post /api/admin/config/smart_connect", "SmartConnectUpdateData"],
    ["get /api/admin/config/fnos_share_bypass", null],
    ["post /api/admin/config/fnos_share_bypass", "FnosShareBypassUpdateData"],
    ["get /api/admin/config/fnos_port_icon_hijack", null],
    [
      "post /api/admin/config/fnos_port_icon_hijack",
      "FnosPortIconHijackUpdateData",
    ],
    ["get /api/admin/config/fnos_network_tuning", null],
    [
      "post /api/admin/config/fnos_network_tuning",
      "FnosNetworkTuningUpdateData",
    ],
    ["get /api/admin/config/fnos_connect_waf", null],
    ["post /api/admin/config/fnos_connect_waf", "FnosConnectWafUpdateData"],
    ["get /api/admin/config/fnos_certificate_sync/details", null],
    [
      "post /api/admin/config/fnos_certificate_sync",
      "FnosCertificateSyncUpdateData",
    ],
    [
      "post /api/admin/config/fnos_certificate_sync/sync",
      "FnosCertificateSyncBodyData",
    ],
    ["get /api/admin/config/dashboard_display", null],
    ["post /api/admin/config/dashboard_display", "DashboardDisplayUpdateData"],
    ["get /api/admin/dashboard/stats", null],
    ["get /api/admin/dashboard/realtime", null],
    ["get /api/admin/dashboard/active-ips", null],
    ["get /api/admin/update/status", null],
    ["post /api/admin/update/check", null],
    ["post /api/admin/update/check-and-download", null],
    ["post /api/admin/update/download", null],
    ["post /api/admin/update/install", null],
    ["get /api/admin/update/confirm", null],
    ["get /api/admin/cidr/capabilities", null],
    ["get /api/admin/cidr/provinces", null],
    ["get /api/admin/cidr/cities", null],
    ["get /api/admin/cidr/selector", null],
    ["get /api/admin/cidr/cidrs", null],
    ["post /api/admin/ip-location/batch", "IpLocationBatchBodyData"],
    ["get /api/admin/config/ip_location_api", null],
    ["post /api/admin/config/ip_location_api", "IpLocationApiConfigData"],
    [
      "post /api/admin/config/ip_location_api/test-ip-lookup",
      "IpLocationTestUrlBodyData",
    ],
    [
      "post /api/admin/config/ip_location_api/test-cidr",
      "IpLocationTestUrlBodyData",
    ],
    ["get /api/admin/runtime-health", null],
    ["get /api/admin/runtime-health/gateway-memory", null],
    [
      "put /api/admin/runtime-health/gateway-memory",
      "GatewayMemoryConfigUpdateData",
    ],
    [
      "post /api/admin/runtime-health/gateway-memory/reclaim",
      "GatewayMemoryReclaimBodyData",
    ],
    ["get /api/admin/runtime-health/logs/{component}", null],
    ["delete /api/admin/runtime-health/logs/{component}", null],
    ["get /api/admin/runtime-health/diagnostics", null],
    ["get /api/admin/runtime-health/diagnostics/archive", null],
    ["get /api/admin/security/overview", null],
    ["get /api/admin/scanner/settings", null],
    ["post /api/admin/scanner/settings", "ScannerSettingsUpdateData"],
    ["get /api/admin/scanner/blacklist", null],
    ["delete /api/admin/scanner/blacklist", "IpListBodyData"],
    ["get /api/admin/scanner/blacklist/{ip}", null],
    ["delete /api/admin/scanner/blacklist/{ip}", null],
    ["get /api/admin/general-blacklist", null],
    ["post /api/admin/general-blacklist", "GeneralBlacklistAddBodyData"],
    ["delete /api/admin/general-blacklist", "IpListBodyData"],
    ["post /api/admin/general-blacklist/status", "IpListBodyData"],
    ["delete /api/admin/general-blacklist/{ip}", null],
    ["get /api/admin/ssh-security/config", null],
    ["post /api/admin/ssh-security/config", "SshSecurityConfigUpdateData"],
    ["post /api/admin/ssh-security/firewall/sync", null],
    ["post /api/admin/ssh-security/firewall/clear", null],
    ["get /api/admin/ssh-security/login-logs", null],
    ["get /api/admin/ssh-security/blocks", null],
    ["delete /api/admin/ssh-security/blocks", "SshBlocksDeleteBodyData"],
    ["get /api/admin/ssh-security/blocks/{ip}", null],
    ["delete /api/admin/ssh-security/blocks/{ip}", null],
    ["get /api/admin/whitelist", null],
    ["post /api/admin/whitelist", "WhitelistAddBodyData"],
    ["get /api/admin/whitelist/regions", null],
    ["post /api/admin/whitelist/regions", "WhitelistRegionAddBodyData"],
    ["delete /api/admin/whitelist/regions/{id}", null],
    ["delete /api/admin/whitelist/{id}", null],
    ["patch /api/admin/whitelist/{id}/comment", "WhitelistCommentBodyData"],
    ["post /api/admin/whitelist/{id}/refresh", null],
    ["get /api/admin/panel/bootstrap", null],
    ["post /api/admin/panel/password", "PanelPasswordBodyData"],
    ["post /api/admin/panel/password/change", "PanelPasswordBodyData"],
    ["post /api/admin/panel/login", "PanelLoginBodyData"],
    ["post /api/admin/panel/logout", null],
    ["get /api/admin/auth/mode", null],
    ["post /api/admin/auth/mode/preview", "AuthLoginModeBody"],
    ["post /api/admin/auth/mode/switch", "AuthLoginModeBody"],
    ["get /api/admin/auth/oidc/catalog", null],
    ["get /api/admin/auth/oidc/providers", null],
    ["post /api/admin/auth/oidc/providers", "OidcProviderCreateData"],
    ["patch /api/admin/auth/oidc/providers/{id}", "OidcProviderUpdateData"],
    ["delete /api/admin/auth/oidc/providers/{id}", null],
    ["post /api/admin/auth/oidc/providers/{id}/test", null],
    ["get /api/admin/auth/oidc/totp/{totp_id}/bindings", null],
    ["delete /api/admin/auth/oidc/bindings/{id}", null],
    ["post /api/admin/auth/oidc/invitations", "ExternalAuthInvitationBodyData"],
    ["get /api/admin/auth/ldap/catalog", null],
    ["get /api/admin/auth/ldap/providers", null],
    ["post /api/admin/auth/ldap/providers", "LdapProviderCreateData"],
    ["patch /api/admin/auth/ldap/providers/{id}", "LdapProviderUpdateData"],
    ["delete /api/admin/auth/ldap/providers/{id}", null],
    [
      "post /api/admin/auth/ldap/providers/{id}/test",
      "LdapProviderTestBodyData",
    ],
    ["get /api/admin/auth/ldap/totp/{totp_id}/bindings", null],
    ["delete /api/admin/auth/ldap/bindings/{id}", null],
    ["post /api/admin/auth/ldap/invitations", "ExternalAuthInvitationBodyData"],
    ["get /api/admin/config/auth_credential_settings", null],
    [
      "post /api/admin/config/auth_credential_settings",
      "AuthCredentialSettingsUpdateData",
    ],
    ["get /api/admin/totp/status", null],
    ["get /api/admin/auth/accounts", null],
    ["post /api/admin/auth/accounts", "AuthAccountCreateBody"],
    ["patch /api/admin/auth/accounts/{id}", "AuthAccountPatchBody"],
    ["delete /api/admin/auth/accounts/{id}", null],
    ["post /api/admin/auth/accounts/{id}/password", "AuthAccountPasswordBody"],
    ["post /api/admin/auth/accounts/{id}/setup", "AuthAccountSetupBody"],
    ["post /api/admin/auth/accounts/{id}/totp/setup", null],
    ["post /api/admin/auth/accounts/{id}/totp/bind", "TotpBindBody"],
    [
      "patch /api/admin/auth/accounts/{id}/access-scopes",
      "AccessScopesUpdateData",
    ],
    [
      "patch /api/admin/auth/accounts/{id}/subdomain-access",
      "SubdomainAccessUpdateData",
    ],
    ["post /api/admin/totp/setup", null],
    ["post /api/admin/totp/bind", "TotpBindBody"],
    ["get /api/admin/totp/credentials/export", null],
    ["post /api/admin/totp/credentials/import", "CredentialImportBodyData"],
    ["delete /api/admin/totp/{id}", null],
    ["patch /api/admin/totp/{id}/access-scopes", "AccessScopesUpdateData"],
    [
      "patch /api/admin/totp/{id}/subdomain-access",
      "SubdomainAccessUpdateData",
    ],
    ["patch /api/admin/totp/{id}/comment", "TotpCommentBody"],
    ["get /api/admin/totp/{totp_id}/passkeys", null],
    ["delete /api/admin/passkeys/{id}", null],
    ["get /api/admin/config", null],
    ["get /api/admin/config/locale", null],
    ["post /api/admin/config/locale", "LocaleConfigData"],
    ["get /api/admin/config/wol_feature", null],
    ["post /api/admin/config/wol_feature", "WolFeatureConfigUpdateData"],
    ["post /api/admin/config/proxy_mappings", "ProxyMappingsUpdateData"],
    ["get /api/admin/config/host_mappings", null],
    ["post /api/admin/config/host_mappings", "MappingsBody"],
    ["get /api/admin/config/host_mapping_catalog", null],
    ["post /api/admin/config/host_mapping_catalog", "HostMappingCatalogBody"],
    [
      "post /api/admin/config/host_mappings/basic_auth_probe",
      "HostMappingBasicAuthProbeBodyData",
    ],
    ["get /api/admin/config/host_mappings/bookmarks/export", null],
    [
      "post /api/admin/config/host_mappings/metadata",
      "HostMappingMetadataBodyData",
    ],
    ["post /api/admin/config/host_mappings/refresh_titles", null],
    ["get /api/admin/config/host_mappings/{host}/advanced_auth", null],
    [
      "put /api/admin/config/host_mappings/{host}/advanced_auth",
      "AdvancedAuthUpdateBodyData",
    ],
    ["get /api/admin/scan/discover-settings", null],
    [
      "post /api/admin/scan/discover-settings",
      "ScanDiscoverySettingsUpdateData",
    ],
    ["get /api/admin/scan/discover-targets", null],
    ["post /api/admin/scan/discover-targets", "ScanDiscoveryTargetsUpdateData"],
    ["post /api/admin/scan/discover/jobs", "ScanDiscoverJobBodyData"],
    ["get /api/admin/scan/discover/jobs/{job_id}", null],
    ["delete /api/admin/scan/discover/jobs/{job_id}", null],
    ["post /api/admin/scan/host-mappings/probe", "HostMappingsProbeBodyData"],
    ["get /api/admin/deep-monitor/sessions", null],
    ["post /api/admin/deep-monitor/sessions", "DeepMonitorStartBodyData"],
    ["get /api/admin/deep-monitor/sessions/{session_id}", null],
    ["delete /api/admin/deep-monitor/sessions/{session_id}", null],
    [
      "post /api/admin/deep-monitor/sessions/{session_id}/extend",
      "DeepMonitorExtendBodyData",
    ],
    ["post /api/admin/deep-monitor/sessions/{session_id}/stop", null],
    ["get /api/admin/deep-monitor/sessions/{session_id}/events", null],
    [
      "get /api/admin/deep-monitor/sessions/{session_id}/events/{event_id}",
      null,
    ],
    [
      "get /api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload",
      null,
    ],
    ["get /api/admin/deep-monitor/sessions/{session_id}/live", null],
    ["get /api/admin/deep-monitor/sessions/{session_id}/download", null],
    ["get /api/admin/config/stream_mappings", null],
    ["post /api/admin/config/stream_mappings", "StreamMappingsUpdateData"],
    ["get /api/admin/config/subdomain_mode", null],
    ["post /api/admin/config/subdomain_mode", "SubdomainModeUpdateData"],
    ["get /api/admin/config/gateway", null],
    ["post /api/admin/config/gateway", "GatewaySettingsUpdateData"],
    ["get /api/admin/config/gateway/visibility", null],
    [
      "post /api/admin/config/gateway/visibility",
      "GatewayVisibilityUpdateData",
    ],
    ["get /api/admin/config/gateway/proxy-headers", null],
    [
      "post /api/admin/config/gateway/proxy-headers",
      "GatewayProxyHeadersUpdateData",
    ],
    ["get /api/admin/config/gateway/host-response", null],
    [
      "post /api/admin/config/gateway/host-response",
      "GatewayHostResponseUpdateData",
    ],
    ["get /api/admin/gateway-logs/config", null],
    ["post /api/admin/gateway-logs/config", "GatewayLoggingConfigUpdateData"],
    ["get /api/admin/gateway-logs/directory", null],
    ["get /api/admin/gateway-logs/dates", null],
    ["get /api/admin/gateway-logs/entries", null],
    ["delete /api/admin/gateway-logs/entries", "GatewayLogDeleteBodyData"],
    ["get /api/admin/gateway-logs/analytics", null],
    ["post /api/admin/gateway-logs/analytics", null],
    ["get /api/admin/wol/local-relay", null],
    ["put /api/admin/wol/local-relay", "WolLocalRelayInputData"],
    ["post /api/admin/wol/local-relay/pair", "WolLocalRelayPairBodyData"],
    ["get /api/admin/wol/relays", null],
    ["post /api/admin/wol/relays", "WolRelayInputData"],
    ["get /api/admin/wol/relays/{id}", null],
    ["put /api/admin/wol/relays/{id}", "WolRelayInputData"],
    ["delete /api/admin/wol/relays/{id}", null],
    ["post /api/admin/wol/relays/{id}/rotate-psk", null],
    ["post /api/admin/wol/relays/{id}/probe", null],
    ["post /api/admin/wol/discover/jobs", "WolDiscoveryBodyData"],
    ["get /api/admin/wol/discover/jobs/{id}", null],
    ["delete /api/admin/wol/discover/jobs/{id}", null],
    ["get /api/admin/wol/targets", null],
    ["post /api/admin/wol/targets", "WolTargetInputData"],
    ["get /api/admin/wol/targets/{id}", null],
    ["put /api/admin/wol/targets/{id}", "WolTargetInputData"],
    ["delete /api/admin/wol/targets/{id}", null],
    ["post /api/admin/wol/targets/{id}/wake", null],
    ["get /api/admin/sessions", null],
    ["get /api/admin/sessions/{id}", null],
    ["delete /api/admin/sessions/{id}", null],
    ["patch /api/admin/sessions/{id}/comment", "SessionCommentBody"],
    ["get /api/admin/sessions/{id}/mobility", null],
    ["get /api/admin/maintenance/backup/automatic", null],
    [
      "put /api/admin/maintenance/backup/automatic",
      "UpdateAutomaticBackupBody",
    ],
    ["get /api/admin/maintenance/backup/automatic/files", null],
    ["get /api/admin/maintenance/backup/export", null],
    ["get /api/admin/maintenance/backup/files", null],
    ["post /api/admin/maintenance/backup/export/fnos", null],
    ["post /api/admin/maintenance/backup/import", "ImportBackupBody"],
    [
      "post /api/admin/maintenance/backup/import/automatic",
      "ImportBackupFromDirectoryBody",
    ],
    [
      "post /api/admin/maintenance/backup/import/fnos",
      "ImportBackupFromDirectoryBody",
    ],
    ["delete /api/admin/acme", null],
    ["get /api/admin/acme/status", null],
    ["get /api/admin/acme/resource/status", null],
    ["post /api/admin/acme/resource/initialize", null],
    ["post /api/admin/acme/resource/cancel", null],
    ["delete /api/admin/acme/resource", null],
    ["get /api/admin/acme/overview", null],
    ["get /api/admin/acme/dns-providers", null],
    ["get /api/admin/acme/subdomain-recommendation", null],
    ["post /api/admin/acme/init", null],
    ["post /api/admin/acme/client-settings", "AcmeClientSettingsBodyData"],
    ["get /api/admin/acme/config", null],
    ["post /api/admin/acme/config", "AcmeConfigBodyData"],
    ["get /api/admin/acme/applications", null],
    ["post /api/admin/acme/applications", "AcmeApplicationBodyData"],
    ["get /api/admin/acme/applications/{id}", null],
    ["patch /api/admin/acme/applications/{id}", "AcmeApplicationBodyData"],
    ["delete /api/admin/acme/applications/{id}", null],
    ["delete /api/admin/acme/applications/{id}/certificate", null],
    ["post /api/admin/acme/applications/{id}/library/sync", null],
    ["post /api/admin/acme/applications/{id}/deploy", null],
    ["post /api/admin/acme/applications/{id}/request", null],
    ["post /api/admin/acme/request", "AcmeLegacyRequestBodyData"],
    ["post /api/admin/acme/jobs/active/stop", null],
    ["get /api/admin/acme/jobs/{id}", null],
    ["get /api/admin/acme/jobs/{id}/logs", null],
    ["get /api/admin/acme/jobs/{id}/poll", null],
    ["get /api/admin/acme/certs/{domain}", null],
    ["delete /api/admin/acme/certs/{domain}", null],
    ["get /api/admin/acme/certs/{domain}/download", null],
    ["post /api/admin/acme/certs/{domain}/deploy", null],
  ]);
  const operations = [];
  const typedDomainOperationsSeen = new Set();
  for (const [route, pathItem] of Object.entries(document.paths ?? {})) {
    for (const method of ["get", "post", "put", "patch", "delete", "head"]) {
      const operation = pathItem[method];
      if (!operation) continue;
      operations.push([method, route]);
      const operationKey = `${method} ${route}`;
      if (operation["x-fn-knock-contract-source"] === "scanner-fallback") {
        throw new Error(
          `${method.toUpperCase()} ${route} uses removed scanner fallback`,
        );
      }
      if (
        !new Set(["utoipa", "utoipa-domain"]).has(
          operation["x-fn-knock-contract-source"],
        )
      ) {
        throw new Error(
          `${method.toUpperCase()} ${route} has an unknown contract source`,
        );
      }
      if (typedDomainOperations.has(operationKey)) {
        typedDomainOperationsSeen.add(operationKey);
        if (
          !new Set(["utoipa", "utoipa-domain"]).has(
            operation["x-fn-knock-contract-source"],
          )
        ) {
          throw new Error(
            `${method.toUpperCase()} ${route} is not generated from a typed contract`,
          );
        }
        const requestSchema = typedDomainOperations.get(operationKey);
        const actualRef =
          operation.requestBody?.content?.["application/json"]?.schema?.$ref;
        if (
          requestSchema &&
          actualRef !== `#/components/schemas/${requestSchema}`
        ) {
          throw new Error(
            `${method.toUpperCase()} ${route} must use ${requestSchema}`,
          );
        }
        if (!requestSchema && actualRef) {
          throw new Error(
            `${method.toUpperCase()} ${route} unexpectedly declares a request body`,
          );
        }
      }
      if (!operation.responses?.["200"] || !operation.responses?.default) {
        throw new Error(
          `${method.toUpperCase()} ${route} lacks success or error schema`,
        );
      }
      if (
        new Set(["post", "put", "patch"]).has(method) &&
        !typedDomainOperations.has(operationKey) &&
        !operation.requestBody?.content?.["application/json"]?.schema
      ) {
        throw new Error(
          `${method.toUpperCase()} ${route} lacks a request schema`,
        );
      }
    }
  }
  if (operations.length !== typedDomainOperations.size + 1) {
    throw new Error(
      `expected ${typedDomainOperations.size + 1} fully typed operations, got ${operations.length}`,
    );
  }
  if (
    document.paths?.["/api/admin/healthz"]?.get?.[
      "x-fn-knock-contract-source"
    ] !== "utoipa"
  ) {
    throw new Error("health check must remain generated by utoipa-axum");
  }
  if (typedDomainOperationsSeen.size !== typedDomainOperations.size) {
    const missing = [...typedDomainOperations.keys()].filter(
      (operation) => !typedDomainOperationsSeen.has(operation),
    );
    throw new Error(
      `typed domain route coverage is incomplete: ${missing.join(", ")}`,
    );
  }
  const eventDeleteSchema =
    document.paths?.["/api/admin/events"]?.delete?.requestBody?.content?.[
      "application/json"
    ]?.schema?.$ref;
  if (eventDeleteSchema !== "#/components/schemas/SystemEventDeleteBodyData") {
    throw new Error("system event deletion must preserve its JSON body");
  }
  const eventParameters =
    document.paths?.["/api/admin/events"]?.get?.parameters ?? [];
  for (const [parameterName, expected] of [
    ["level", ["INFO", "WARN", "ERROR", "CRITICAL"]],
    [
      "source",
      ["SERVER_ADMIN", "GO_REAUTH_PROXY", "SYSTEM_MONITOR", "RUNTIME_MONITOR"],
    ],
  ]) {
    const actual = eventParameters.find(
      (parameter) => parameter.name === parameterName,
    )?.schema?.enum;
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(`system event ${parameterName} filter is out of sync`);
    }
  }
  const internalEventResponse =
    document.paths?.["/api/internal/system-events"]?.post?.responses?.["200"]
      ?.content?.["application/json"]?.schema;
  if (
    internalEventResponse?.$ref !==
      "#/components/schemas/SystemEventPublishResultData" ||
    internalEventResponse?.properties?.data
  ) {
    throw new Error(
      "internal system event publication must remain direct JSON",
    );
  }
  const publishResultRequired =
    document.components?.schemas?.SystemEventPublishResultData?.required ?? [];
  if (!publishResultRequired.includes("data")) {
    throw new Error(
      "internal system event result must always emit nullable data",
    );
  }
  const backoffStatusParameters =
    document.paths?.["/api/admin/backoff/status"]?.get?.parameters ?? [];
  if (
    !backoffStatusParameters.some(
      (parameter) =>
        parameter.name === "ip" &&
        parameter.in === "query" &&
        parameter.required === true,
    )
  ) {
    throw new Error("login backoff status must require the ip query parameter");
  }
  const backoffProperties =
    document.components?.schemas?.LoginBackoffData?.properties ?? {};
  for (const field of [
    "ip",
    "attempts",
    "blocked",
    "retryAfter",
    "blockedUntil",
  ]) {
    if (!(field in backoffProperties)) {
      throw new Error(`login backoff status must preserve ${field}`);
    }
  }
  const captchaUpdate =
    document.components?.schemas?.CaptchaSettingsUpdateData ?? {};
  if ((captchaUpdate.required ?? []).length !== 0) {
    throw new Error("captcha updates must remain partial");
  }
  const captchaPowNumber =
    document.components?.schemas?.CaptchaPowData?.properties?.base_max_number;
  if (
    captchaPowNumber?.minimum !== 10_000 ||
    captchaPowNumber?.maximum !== 1_000_000 ||
    captchaPowNumber?.multipleOf !== 10_000
  ) {
    throw new Error("captcha proof-of-work bounds are out of sync");
  }
  const captchaSecret =
    document.components?.schemas?.CaptchaTurnstileData?.properties?.secret_key;
  if (captchaSecret?.format !== "password" || captchaSecret?.writeOnly) {
    throw new Error(
      "management captcha secret must be marked sensitive without contradicting its response shape",
    );
  }
  const runTypes =
    document.components?.schemas?.RunTypeUpdateData?.properties?.run_type?.enum;
  if (JSON.stringify(runTypes) !== JSON.stringify([0, 1, 3])) {
    throw new Error("run type values are out of sync");
  }
  const terminalUpdate =
    document.components?.schemas?.TerminalFeatureUpdateData ?? {};
  if (
    (terminalUpdate.required ?? []).length !== 0 ||
    terminalUpdate.properties?.resume_backend
  ) {
    throw new Error(
      "terminal updates must remain partial and exclude the runtime-owned backend",
    );
  }
  const terminalData =
    document.components?.schemas?.TerminalFeatureData?.properties ?? {};
  if (
    terminalData.max_sessions?.minimum !== 1 ||
    terminalData.max_sessions?.maximum !== 12 ||
    terminalData.idle_timeout_seconds?.minimum !== 60 ||
    terminalData.idle_timeout_seconds?.maximum !== 604_800
  ) {
    throw new Error("terminal feature bounds are out of sync");
  }
  const welcomeRequired =
    document.components?.schemas?.WelcomeGuideData?.required ?? [];
  if (!welcomeRequired.includes("completed_at")) {
    throw new Error("welcome guide must always emit nullable completed_at");
  }
  const appearancePresets =
    document.components?.schemas?.PanelAppearanceData?.properties
      ?.theme_color_preset?.enum;
  if (
    JSON.stringify(appearancePresets) !==
    JSON.stringify([
      "default",
      "hermes_orange",
      "prussian_blue",
      "dynamic_white",
    ])
  ) {
    throw new Error("appearance presets are out of sync");
  }
  const autoHttpsRuntime =
    document.components?.schemas?.AutoHttpsRuntimeData ?? {};
  for (const field of ["last_error", "last_error_at"]) {
    if (!(autoHttpsRuntime.required ?? []).includes(field)) {
      throw new Error(`auto HTTPS runtime must always emit nullable ${field}`);
    }
  }
  if (autoHttpsRuntime.properties?.listen_port?.const !== 80) {
    throw new Error("auto HTTPS redirect port is out of sync");
  }
  const tunnelTypes =
    document.components?.schemas?.DefaultTunnelUpdateData?.properties?.tunnel
      ?.enum;
  if (JSON.stringify(tunnelTypes) !== JSON.stringify(["frp", "cloudflared"])) {
    throw new Error("default tunnel values are out of sync");
  }
  const firewallPorts =
    document.components?.schemas?.FirewallAdditionalPortsUpdateData?.properties
      ?.ports;
  if (
    firewallPorts?.maxItems !== 128 ||
    firewallPorts?.uniqueItems !== true ||
    firewallPorts?.items?.minimum !== 1 ||
    firewallPorts?.items?.maximum !== 65535
  ) {
    throw new Error("firewall additional-port bounds are out of sync");
  }
  const firewallResetRunType =
    document.components?.schemas?.FirewallResetBodyData?.properties?.run_type
      ?.enum;
  const firewallResetGatewayPort =
    document.components?.schemas?.FirewallResetData?.properties?.gatewayPort;
  if (
    JSON.stringify(firewallResetRunType) !== JSON.stringify([0, 1, 3]) ||
    firewallResetGatewayPort?.minimum !== 1 ||
    firewallResetGatewayPort?.maximum !== 65_535
  ) {
    throw new Error("firewall reset contract is out of sync");
  }
  const syncRoutesRequired =
    document.components?.schemas?.SyncRoutesData?.required ?? [];
  for (const field of [
    "synced_rules",
    "synced_host_rules",
    "synced_stream_rules",
    "synced_gateway_logging",
    "synced_waf",
    "waf_bundle_id",
  ]) {
    if (!syncRoutesRequired.includes(field)) {
      throw new Error(`route sync must always emit ${field}`);
    }
  }
  const maintenanceClearRequired =
    document.components?.schemas?.MaintenanceClearData?.required ?? [];
  if (!maintenanceClearRequired.includes("gateway_reset")) {
    throw new Error("maintenance clear must confirm the gateway reset");
  }
  const accessEntryEnv =
    document.components?.schemas?.AccessEntryData?.properties?.env?.enum;
  if (
    JSON.stringify(accessEntryEnv) !==
    JSON.stringify(["GO_REPROXY_PORT", "FRP_REMOTE_PORT"])
  ) {
    throw new Error("access-entry source values are out of sync");
  }
  const clockStatus = document.components?.schemas?.SystemClockStatusData ?? {};
  const clockStatusRequired = clockStatus.required ?? [];
  for (const field of [
    "systemTimeZone",
    "checkedAt",
    "networkSource",
    "lastCheckError",
    "systemTimeMs",
    "remoteTimeMs",
    "systemBeijingTime",
    "remoteBeijingTime",
    "driftMs",
    "lastSyncAt",
    "lastSyncError",
    "syncSummary",
  ]) {
    if (!clockStatusRequired.includes(field)) {
      throw new Error(`system clock must always emit nullable ${field}`);
    }
  }
  if (
    clockStatus.properties?.expectedTimeZone?.const !== "Asia/Shanghai" ||
    clockStatus.properties?.driftThresholdMs?.const !== 90_000 ||
    JSON.stringify(
      document.components?.schemas?.SystemClockIssueData?.properties?.code
        ?.enum,
    ) !== JSON.stringify(["timezone_mismatch", "time_mismatch"])
  ) {
    throw new Error("system clock constants are out of sync");
  }
  const clockSyncSchema =
    document.paths?.["/api/admin/system/clock/sync"]?.post?.responses?.["200"]
      ?.content?.["application/json"]?.schema;
  if (
    clockSyncSchema?.$ref !== "#/components/schemas/SystemClockSyncResponseData"
  ) {
    throw new Error("system clock sync response envelope is out of sync");
  }
  const assetProgress =
    document.components?.schemas?.SystemAssetDownloadProgressData ?? {};
  if (
    JSON.stringify(assetProgress.properties?.status?.enum) !==
      JSON.stringify(["idle", "downloading", "completed", "error"]) ||
    assetProgress.properties?.percent?.minimum !== 0 ||
    assetProgress.properties?.percent?.maximum !== 100 ||
    !(assetProgress.required ?? []).includes("error")
  ) {
    throw new Error("system asset progress contract is out of sync");
  }
  const cloudflaredPlatforms =
    document.components?.schemas?.CloudflaredAssetStatusData?.properties
      ?.platform?.enum;
  const frpPlatforms =
    document.components?.schemas?.FrpAssetStatusData?.properties?.platform
      ?.enum;
  if (
    !cloudflaredPlatforms?.includes("windows-amd64") ||
    !cloudflaredPlatforms?.includes("linux-armhf") ||
    frpPlatforms?.includes("windows-amd64") ||
    !frpPlatforms?.includes("unsupported")
  ) {
    throw new Error("system asset platform contracts are out of sync");
  }
  for (const [method, route] of [
    ["post", "/api/admin/system/cloudflared/download"],
    ["post", "/api/admin/system/cloudflared/cancel"],
    ["delete", "/api/admin/system/cloudflared"],
    ["post", "/api/admin/system/frp/download"],
    ["post", "/api/admin/system/frp/cancel"],
    ["delete", "/api/admin/system/frp"],
  ]) {
    const schema =
      document.paths?.[route]?.[method]?.responses?.["200"]?.content?.[
        "application/json"
      ]?.schema;
    if (
      schema?.$ref !== "#/components/schemas/SystemAssetMutationResponseData"
    ) {
      throw new Error(`${method.toUpperCase()} ${route} message is not typed`);
    }
  }
  const dnsmasqState =
    document.components?.schemas?.DnsmasqInstallStateData ?? {};
  if (
    JSON.stringify(dnsmasqState.properties?.status?.enum) !==
      JSON.stringify(["uninstalled", "installing", "installed", "error"]) ||
    dnsmasqState.properties?.progress?.minimum !== 0 ||
    dnsmasqState.properties?.progress?.maximum !== 100
  ) {
    throw new Error("dnsmasq install-state contract is out of sync");
  }
  const terminalTmux =
    document.components?.schemas?.TerminalTmuxInstallStateData ?? {};
  if (
    JSON.stringify(terminalTmux.properties?.status?.enum) !==
      JSON.stringify(["uninstalled", "installing", "installed", "error"]) ||
    terminalTmux.properties?.progress?.minimum !== 0 ||
    terminalTmux.properties?.progress?.maximum !== 100 ||
    JSON.stringify(terminalTmux.properties?.detectionSource?.enum) !==
      JSON.stringify(["env-path", "absolute-path", null]) ||
    !(terminalTmux.required ?? []).includes("detectionSource")
  ) {
    throw new Error("terminal tmux state contract is out of sync");
  }
  const terminalRuntime =
    document.components?.schemas?.TerminalRuntimeStatusData ?? {};
  if (
    terminalRuntime.properties?.httpPollingAvailable?.const !== true ||
    !(terminalRuntime.required ?? []).includes("tmuxDetectionSource")
  ) {
    throw new Error("terminal runtime capability contract is out of sync");
  }
  const terminalSession =
    document.components?.schemas?.TerminalSessionData ?? {};
  if (
    JSON.stringify(terminalSession.properties?.status?.enum) !==
      JSON.stringify(["created", "attached", "detached", "stopped", "error"]) ||
    terminalSession.properties?.cols?.minimum !== 20 ||
    terminalSession.properties?.cols?.maximum !== 400 ||
    terminalSession.properties?.rows?.minimum !== 8 ||
    terminalSession.properties?.rows?.maximum !== 200 ||
    terminalSession.properties?.resume_backend?.const !== "tmux" ||
    !(terminalSession.required ?? []).includes("last_frame_revision")
  ) {
    throw new Error("terminal session contract is out of sync");
  }
  if (
    document.components?.schemas?.TerminalAttachmentData?.properties?.transport
      ?.const !== "http-polling" ||
    document.components?.schemas?.TerminalOutputChunkData?.properties?.cursor
      ?.minimum !== 0 ||
    !(
      document.components?.schemas?.TerminalPollResultData?.required ?? []
    ).includes("chunk")
  ) {
    throw new Error("terminal attachment polling contract is out of sync");
  }
  const terminalPollParameters =
    document.paths?.["/api/admin/terminal/attachments/{id}/poll"]?.get
      ?.parameters ?? [];
  const terminalCursor = terminalPollParameters.find(
    (parameter) => parameter.name === "cursor",
  )?.schema;
  const terminalTimeout = terminalPollParameters.find(
    (parameter) => parameter.name === "timeout_ms",
  )?.schema;
  if (
    terminalCursor?.default !== 0 ||
    terminalCursor?.oneOf?.[0]?.minimum !== 0 ||
    terminalCursor?.oneOf?.[1]?.pattern !== "^\\s*[+-]?\\d+" ||
    terminalTimeout?.default !== 15_000
  ) {
    throw new Error("terminal long-poll query contract is out of sync");
  }
  const cloudflaredConfig =
    document.components?.schemas?.CloudflaredConfigData ?? {};
  const cloudflaredConfigUpdate =
    document.components?.schemas?.CloudflaredConfigUpdateData ?? {};
  if (
    JSON.stringify(cloudflaredConfig.properties?.mode?.enum) !==
      JSON.stringify(["manual", "managed"]) ||
    JSON.stringify(cloudflaredConfig.properties?.protocol?.enum) !==
      JSON.stringify(["auto", "http2", "quic"]) ||
    !(cloudflaredConfig.required ?? []).includes("rootDomain") ||
    (cloudflaredConfigUpdate.required ?? []).length !== 0 ||
    cloudflaredConfigUpdate.properties?.token?.writeOnly !== true
  ) {
    throw new Error("cloudflared configuration contract is out of sync");
  }
  const cloudflaredFailure =
    document.components?.schemas?.CloudflaredSupervisorFailureData ?? {};
  if (
    !(cloudflaredFailure.required ?? []).includes("resources") ||
    cloudflaredFailure.properties?.at?.format !== "date-time" ||
    JSON.stringify(
      document.components?.schemas?.CloudflaredSupervisorData?.properties?.state
        ?.enum,
    ) !== JSON.stringify(["stopped", "starting", "running", "backoff"])
  ) {
    throw new Error("cloudflared supervisor contract is out of sync");
  }
  const reconcileRequest =
    document.components?.schemas?.CloudflareReconcileRequestData ?? {};
  const reconcilePlan =
    document.components?.schemas?.CloudflareReconcilePlanData ?? {};
  if (
    (reconcileRequest.required ?? []).length !== 0 ||
    JSON.stringify(reconcileRequest.properties?.action?.enum) !==
      JSON.stringify(["apply", "cleanup"]) ||
    JSON.stringify(reconcileRequest.properties?.tunnelMode?.enum) !==
      JSON.stringify(["dedicated", "existing"]) ||
    reconcilePlan.properties?.remoteFingerprint?.pattern !== "^[0-9a-f]{64}$"
  ) {
    throw new Error("cloudflared reconcile contract is out of sync");
  }
  const optimizationScan =
    document.components?.schemas?.CloudflareOptimizationScanData ?? {};
  const optimizationSources =
    document.components?.schemas
      ?.CloudflareOptimizationSourceSettingsBodyData ?? {};
  if (
    JSON.stringify(optimizationScan.properties?.status?.enum) !==
      JSON.stringify([
        "queued",
        "running",
        "completed",
        "failed",
        "cancelled",
      ]) ||
    optimizationScan.properties?.progress?.minimum !== 0 ||
    optimizationScan.properties?.progress?.maximum !== 100 ||
    optimizationSources.properties?.customHostnames?.maxItems !== 16 ||
    optimizationSources.properties?.customHostnames?.uniqueItems !== true
  ) {
    throw new Error("cloudflared optimization contract is out of sync");
  }
  const cloudflaredLogParameters =
    document.paths?.["/api/admin/cloudflared/logs"]?.get?.parameters ?? [];
  const cloudflaredPollParameters =
    document.paths?.["/api/admin/cloudflared/poll"]?.get?.parameters ?? [];
  const cloudflaredLogLimit = cloudflaredLogParameters.find(
    (parameter) => parameter.name === "limit",
  )?.schema;
  const cloudflaredPollCursor = cloudflaredPollParameters.find(
    (parameter) => parameter.name === "cursor",
  )?.schema;
  if (
    cloudflaredLogLimit?.default !== 200 ||
    cloudflaredLogLimit?.oneOf?.[1]?.pattern !== "^\\s*[+-]?\\d+" ||
    cloudflaredPollCursor?.oneOf?.[0]?.minimum !== 0 ||
    cloudflaredPollCursor?.oneOf?.[1]?.pattern !== "^[0-9]+$"
  ) {
    throw new Error("cloudflared log query contract is out of sync");
  }
  const frpcStatus = document.components?.schemas?.FrpcStatusData ?? {};
  const frpcInstance =
    document.components?.schemas?.FrpcInstanceStatusData ?? {};
  const frpcOverview =
    document.components?.schemas?.FrpcInstancesOverviewData ?? {};
  const frpcInstanceBody =
    document.components?.schemas?.FrpcInstanceBodyData ?? {};
  if (
    JSON.stringify(frpcStatus.properties?.platform?.enum) !==
      JSON.stringify([
        "darwin-amd64",
        "darwin-arm64",
        "linux-amd64",
        "linux-arm64",
        "linux-arm",
        "unsupported",
      ]) ||
    !(frpcStatus.required ?? []).includes("pid") ||
    !(frpcStatus.required ?? []).includes("config_path") ||
    !(frpcStatus.required ?? []).includes("running_count") ||
    !["pid", "startedAt", "stoppedAt", "lastExitCode", "lastMessage"].every(
      (field) => (frpcInstance.required ?? []).includes(field),
    ) ||
    frpcOverview.properties?.primaryInstanceId?.const !== "primary" ||
    frpcOverview.properties?.extraCount?.maximum !== 20 ||
    (frpcInstanceBody.required ?? []).length !== 0 ||
    frpcInstanceBody.properties?.content?.writeOnly !== true ||
    document.components?.schemas?.FrpcConfigUpdateData?.properties?.content
      ?.writeOnly !== true
  ) {
    throw new Error("FRPC status and configuration contracts are out of sync");
  }
  const frpcLimitParameters =
    document.paths?.["/api/admin/frpc/instances/{id}/logs"]?.get?.parameters ??
    [];
  const frpcPollParameters =
    document.paths?.["/api/admin/frpc/instances/{id}/poll"]?.get?.parameters ??
    [];
  const frpcLimit = frpcLimitParameters.find(
    (parameter) => parameter.name === "limit",
  )?.schema;
  const frpcCursor = frpcPollParameters.find(
    (parameter) => parameter.name === "cursor",
  )?.schema;
  const frpcId = frpcPollParameters.find(
    (parameter) => parameter.name === "id" && parameter.in === "path",
  )?.schema;
  if (
    frpcLimit?.default !== 200 ||
    frpcLimit?.oneOf?.[1]?.pattern !== "^\\s*[+-]?\\d+" ||
    frpcCursor?.oneOf?.[0]?.minimum !== 0 ||
    frpcCursor?.oneOf?.[1]?.pattern !== "^[0-9]+$" ||
    frpcId?.pattern !== "^[A-Za-z0-9-]{1,80}$" ||
    document.paths?.["/api/admin/frpc/poll"]?.get?.responses?.["200"]
      ?.content?.["application/json"]?.schema?.properties?.data?.$ref !==
      "#/components/schemas/FrpcPollData"
  ) {
    throw new Error(
      "FRPC log and path compatibility contracts are out of sync",
    );
  }
  const ddnsSettings = document.components?.schemas?.DdnsSettingsData ?? {};
  const ddnsStatus = document.components?.schemas?.DdnsStatusData ?? {};
  const ddnsTarget = document.components?.schemas?.DdnsTargetDetailData ?? {};
  const ddnsSelector =
    document.components?.schemas?.DdnsInterfaceSelectorData ?? {};
  const ddnsConfigBody = document.components?.schemas?.DdnsConfigBodyData ?? {};
  const ddnsTargetBody = document.components?.schemas?.DdnsTargetBodyData ?? {};
  const ddnsSettingsUpdate =
    document.components?.schemas?.DdnsSettingsUpdateData ?? {};
  if (
    ddnsSettings.properties?.updateIntervalMinutes?.minimum !== 5 ||
    ddnsSettings.properties?.updateIntervalMinutes?.maximum !== 1440 ||
    JSON.stringify(ddnsSettings.properties?.httpTransport?.enum) !==
      JSON.stringify(["curl", "node"]) ||
    JSON.stringify(ddnsSettings.properties?.publicDnsProvider?.enum) !==
      JSON.stringify(["none", "alidns", "tencent", "cloudflare", "google"]) ||
    !ddnsSettingsUpdate.properties?.httpTransport?.enum?.includes("fetch") ||
    !["provider", "primaryTargetId"].every((field) =>
      (ddnsStatus.required ?? []).includes(field),
    ) ||
    !["provider", "lastIP", "selectionAnchor", "lastCheck", "config"].every(
      (field) => (ddnsTarget.required ?? []).includes(field),
    ) ||
    ddnsSelector.properties?.version?.const !== 1 ||
    JSON.stringify(ddnsSelector.properties?.mode?.enum) !==
      JSON.stringify(["auto", "rules"]) ||
    ddnsConfigBody.properties?.config?.writeOnly !== true ||
    ddnsTargetBody.properties?.config?.writeOnly !== true ||
    ddnsTargetBody.properties?.provider?.oneOf?.[1]?.pattern !==
      "^\\s*(?:alidns|baiducloud|cloudflare|dnshe|dnspod|duckdns|dynu|dynv6|edgeone_cname|edgeone|esa|godaddy|huaweicloud|noip|porkbun|tencentcloud)\\s*$" ||
    (ddnsTargetBody.required ?? []).includes("config")
  ) {
    throw new Error("DDNS status and configuration contracts are out of sync");
  }
  const ddnsLogParameters =
    document.paths?.["/api/admin/ddns/logs"]?.get?.parameters ?? [];
  const ddnsPollParameters =
    document.paths?.["/api/admin/ddns/poll"]?.get?.parameters ?? [];
  const ddnsTargetParameters =
    document.paths?.["/api/admin/ddns/targets/{id}/test"]?.post?.parameters ??
    [];
  const ddnsProviderReadParameters =
    document.paths?.["/api/admin/ddns/config/{provider}"]?.get?.parameters ??
    [];
  const ddnsProviderWriteParameters =
    document.paths?.["/api/admin/ddns/config/{provider}"]?.post?.parameters ??
    [];
  const ddnsLogLimit = ddnsLogParameters.find(
    (parameter) => parameter.name === "limit",
  )?.schema;
  const ddnsCursor = ddnsPollParameters.find(
    (parameter) => parameter.name === "cursor",
  )?.schema;
  const ddnsTargetId = ddnsTargetParameters.find(
    (parameter) => parameter.name === "id" && parameter.in === "path",
  )?.schema;
  const ddnsProviderRead = ddnsProviderReadParameters.find(
    (parameter) => parameter.name === "provider" && parameter.in === "path",
  )?.schema;
  const ddnsProviderWrite = ddnsProviderWriteParameters.find(
    (parameter) => parameter.name === "provider" && parameter.in === "path",
  )?.schema;
  if (
    ddnsLogLimit?.default !== 200 ||
    ddnsLogLimit?.oneOf?.[1]?.pattern !== "^\\s*[+-]?\\d+" ||
    ddnsCursor?.oneOf?.[0]?.minimum !== 0 ||
    ddnsCursor?.oneOf?.[1]?.pattern !== "^[0-9]+$" ||
    ddnsTargetId?.pattern !== "^[A-Za-z0-9-]{1,80}$" ||
    ddnsProviderRead?.minLength !== 1 ||
    !ddnsProviderWrite?.oneOf?.[0]?.enum?.includes("cloudflare") ||
    !ddnsProviderWrite?.oneOf?.[1]?.pattern?.startsWith("^\\s*") ||
    document.paths?.["/api/admin/ddns/poll"]?.get?.responses?.["200"]
      ?.content?.["application/json"]?.schema?.properties?.data?.$ref !==
      "#/components/schemas/DdnsPollData" ||
    document.paths?.["/api/admin/ddns/test"]?.post?.responses?.["200"]
      ?.content?.["application/json"]?.schema?.$ref !==
      "#/components/schemas/DdnsTestResponseData"
  ) {
    throw new Error(
      "DDNS query and path compatibility contracts are out of sync",
    );
  }
  const acmeApplicationBody =
    document.components?.schemas?.AcmeApplicationBodyData ?? {};
  const acmeApplication =
    document.components?.schemas?.AcmeApplicationData ?? {};
  const acmeConfigBody = document.components?.schemas?.AcmeConfigBodyData ?? {};
  const acmeStatus = document.components?.schemas?.AcmeStatusData ?? {};
  const acmeJob = document.components?.schemas?.AcmeJobData ?? {};
  const acmePoll = document.components?.schemas?.AcmeJobPollData ?? {};
  if (
    acmeApplicationBody.properties?.credentials?.writeOnly !== true ||
    acmeApplicationBody.properties?.domains?.minItems !== 1 ||
    acmeConfigBody.properties?.credentials?.writeOnly !== true ||
    acmeApplication.properties?.credentials?.writeOnly === true ||
    !acmeApplicationBody.allOf?.[0]?.oneOf?.some((branch) =>
      branch.required?.includes("dnsType"),
    ) ||
    !acmeApplicationBody.allOf?.[0]?.oneOf?.some((branch) =>
      branch.required?.includes("provider"),
    ) ||
    JSON.stringify(acmeStatus.properties?.status?.enum) !==
      JSON.stringify(["uninstalled", "installing", "installed", "error"]) ||
    !(acmeStatus.required ?? []).includes("acmeCert") ||
    JSON.stringify(acmeJob.properties?.status?.enum) !==
      JSON.stringify(["queued", "running", "succeeded", "failed", "stopped"]) ||
    !(acmePoll.required ?? []).includes("analysis")
  ) {
    throw new Error("ACME application and lifecycle contracts are out of sync");
  }
  const acmePollParameters =
    document.paths?.["/api/admin/acme/jobs/{id}/poll"]?.get?.parameters ?? [];
  const acmePollLimit = acmePollParameters.find(
    (parameter) => parameter.name === "limit",
  )?.schema;
  const acmePollOrder = acmePollParameters.find(
    (parameter) => parameter.name === "order",
  )?.schema;
  const acmeJobId = acmePollParameters.find(
    (parameter) => parameter.name === "id" && parameter.in === "path",
  )?.schema;
  const acmeDownload =
    document.paths?.["/api/admin/acme/certs/{domain}/download"]?.get
      ?.responses?.["200"];
  if (
    acmePollLimit?.default !== 500 ||
    acmePollLimit?.oneOf?.[1]?.type !== "string" ||
    acmePollLimit?.oneOf?.[1]?.pattern !== undefined ||
    acmePollOrder?.type !== "string" ||
    acmePollOrder?.enum !== undefined ||
    acmeJobId?.minLength !== 1 ||
    !acmeDownload?.content?.["application/zip"] ||
    acmeDownload?.headers?.["Content-Disposition"]?.schema?.type !== "string" ||
    document.paths?.["/api/admin/acme/certs/{domain}/download"]?.get
      ?.responses?.["204"] !== undefined
  ) {
    throw new Error("ACME poll, path, and download contracts are out of sync");
  }
  const sslSave =
    document.components?.schemas?.SslCertificateSaveBodyData ?? {};
  const sslStatus = document.components?.schemas?.SslStatusData ?? {};
  const sslCoverage =
    document.components?.schemas?.SslSubdomainCoverageData ?? {};
  if (
    !(sslSave.required ?? []).includes("cert") ||
    !(sslSave.required ?? []).includes("key") ||
    sslSave.properties?.key?.writeOnly !== true ||
    (sslSave.required ?? []).includes("activate") ||
    JSON.stringify(sslSave.properties?.source?.enum) !==
      JSON.stringify(["manual", "acme", "ca"]) ||
    JSON.stringify(sslStatus.properties?.deploymentMode?.enum) !==
      JSON.stringify(["single_active", "multi_sni"]) ||
    !["subdomain_coverage", "library_coverage", "gateway_status"].every(
      (field) => (sslStatus.required ?? []).includes(field),
    ) ||
    JSON.stringify(sslCoverage.properties?.status?.enum) !==
      JSON.stringify(["ready", "partial", "missing"]) ||
    !(sslCoverage.required ?? []).includes("auth_host")
  ) {
    throw new Error("SSL certificate and status contracts are out of sync");
  }
  const sharedFileParameters =
    document.paths?.["/api/admin/ssl/shared-files/content"]?.get?.parameters ??
    [];
  const sharedPath = sharedFileParameters.find(
    (parameter) => parameter.name === "path" && parameter.in === "query",
  );
  const caHostsDelete =
    document.paths?.["/api/admin/ssl/ca/hosts"]?.delete ?? {};
  const caHostsDeleteResponse =
    caHostsDelete.responses?.["200"]?.content?.["application/json"]?.schema ??
    {};
  const certificateId = document.paths?.[
    "/api/admin/ssl/certificates/{id}"
  ]?.delete?.parameters?.find(
    (parameter) => parameter.name === "id" && parameter.in === "path",
  );
  if (
    sharedPath?.required !== true ||
    sharedPath?.schema?.minLength !== 1 ||
    caHostsDelete.requestBody?.required !== false ||
    (caHostsDeleteResponse.required ?? []).includes("data") ||
    caHostsDeleteResponse.properties?.data?.items?.type !== "string" ||
    certificateId?.schema?.minLength !== 1
  ) {
    throw new Error("SSL query and compatibility contracts are out of sync");
  }
  for (const route of [
    "/api/admin/ssl/cert.pem",
    "/api/admin/ssl/ca/cert.pem",
  ]) {
    if (
      !document.paths?.[route]?.get?.responses?.["200"]?.content?.[
        "application/x-pem-file"
      ]
    ) {
      throw new Error(`${route} must remain a PEM attachment`);
    }
  }
  for (const route of [
    "/api/admin/ssl/cert.zip",
    "/api/admin/ssl/ca/server-cert.zip",
  ]) {
    if (
      !document.paths?.[route]?.get?.responses?.["200"]?.content?.[
        "application/zip"
      ]
    ) {
      throw new Error(`${route} must remain a ZIP attachment`);
    }
  }
  const wafConfig = document.components?.schemas?.WafConfigData ?? {};
  const wafConfigUpdate =
    document.components?.schemas?.WafConfigUpdateData ?? {};
  if (
    wafConfig.properties?.mode?.const !== "blocking" ||
    wafConfig.properties?.active_bundle_id?.const !== "local" ||
    wafConfig.properties?.paranoia_level?.minimum !== 1 ||
    wafConfig.properties?.paranoia_level?.maximum !== 4 ||
    wafConfig.properties?.executing_paranoia_level?.minimum !== 1 ||
    wafConfig.properties?.executing_paranoia_level?.maximum !== 4 ||
    (wafConfigUpdate.required ?? []).length !== 0 ||
    JSON.stringify(Object.keys(wafConfigUpdate.properties ?? {}).sort()) !==
      JSON.stringify(
        [
          "common_location_exempt_enabled",
          "enabled",
          "executing_paranoia_level",
          "paranoia_level",
          "system_rules_auto_update_enabled",
        ].sort(),
      )
  ) {
    throw new Error("WAF configuration contract is out of sync");
  }
  const wafDetails = document.components?.schemas?.WafDetailsData ?? {};
  const wafSystem = document.components?.schemas?.WafSystemDetailsData ?? {};
  if (
    !(wafDetails.required ?? []).includes("status") ||
    ![
      "manifest",
      "manifest_cached_at",
      "manifest_last_checked_at",
      "manifest_last_error",
      "synced",
    ].every((field) => (wafSystem.required ?? []).includes(field)) ||
    JSON.stringify(
      document.components?.schemas?.WafRuleFileData?.properties?.source?.enum,
    ) !== JSON.stringify(["system", "custom"])
  ) {
    throw new Error("WAF details and rule-file contract is out of sync");
  }
  const wafUpload = document.components?.schemas?.WafUploadBodyData ?? {};
  const wafUploadFile = document.components?.schemas?.WafUploadFileData ?? {};
  const wafRuleSource = document.paths?.[
    "/api/admin/waf/rules/{source}/{filename}"
  ]?.get?.parameters?.find((parameter) => parameter.name === "source")?.schema;
  if (
    wafUpload.properties?.files?.minItems !== 1 ||
    wafUploadFile.properties?.content_base64?.format !== "byte" ||
    JSON.stringify(wafRuleSource?.enum) !== JSON.stringify(["system", "custom"])
  ) {
    throw new Error("WAF upload and rule-source contract is out of sync");
  }
  const wafDrain = document.components?.schemas?.WafDrainResultData ?? {};
  if (
    !(wafDrain.required ?? []).includes("drained") ||
    !(wafDrain.required ?? []).includes("remaining") ||
    Object.hasOwn(wafDrain.properties ?? {}, "events")
  ) {
    throw new Error("WAF drain response must match the persisted-event API");
  }
  const wafLogParameters =
    document.paths?.["/api/admin/waf/logs"]?.get?.parameters ?? [];
  const wafLogCursor = wafLogParameters.find(
    (parameter) => parameter.name === "cursor",
  )?.schema;
  const wafLogLimit = wafLogParameters.find(
    (parameter) => parameter.name === "limit",
  )?.schema;
  if (
    wafLogCursor?.default !== "0" ||
    wafLogCursor?.pattern !== "^\\s*[+-]?\\d+" ||
    wafLogLimit?.default !== "50" ||
    wafLogLimit?.pattern !== "^\\s*[+-]?\\d+"
  ) {
    throw new Error("WAF log pagination compatibility is out of sync");
  }
  const notificationProviderTypes = [
    "wxpusher",
    "serverchan",
    "pushplus",
    "wecom",
    "dingtalk",
    "feishu",
    "email",
    "webhook",
    "pushdeer",
    "harmonyosmeow",
    "magicpush",
    "bark",
    "telegram",
  ];
  const notificationProviderCreate =
    document.components?.schemas?.NotificationProviderCreateBodyData ?? {};
  const notificationProviderUpdate =
    document.components?.schemas?.NotificationProviderUpdateBodyData ?? {};
  if (
    JSON.stringify(notificationProviderCreate.properties?.type?.enum) !==
      JSON.stringify(notificationProviderTypes) ||
    notificationProviderCreate.properties?.connection_config?.writeOnly !==
      true ||
    notificationProviderUpdate.properties?.connection_config?.writeOnly !==
      true ||
    Object.hasOwn(notificationProviderUpdate.properties ?? {}, "type") ||
    (notificationProviderUpdate.required ?? []).length !== 0
  ) {
    throw new Error("notification provider write contract is out of sync");
  }
  const notificationProvider =
    document.components?.schemas?.NotificationProviderData ?? {};
  const notificationProviderDetail =
    document.components?.schemas?.NotificationProviderDetailData ?? {};
  if (
    !["last_test_at", "last_test_status", "last_error"].every((field) =>
      (notificationProvider.required ?? []).includes(field),
    ) ||
    notificationProviderDetail.properties?.connection_config?.writeOnly === true
  ) {
    throw new Error("notification provider read contract is out of sync");
  }
  for (const route of [
    "/api/admin/notifications/providers/test",
    "/api/admin/notifications/providers/{id}/test",
  ]) {
    const schema =
      document.paths?.[route]?.post?.responses?.["200"]?.content?.[
        "application/json"
      ]?.schema;
    if (
      schema?.$ref !==
      "#/components/schemas/NotificationProviderTestResponseData"
    ) {
      throw new Error(
        `notification provider test response is out of sync: ${route}`,
      );
    }
  }
  const notificationRuleCreate =
    document.components?.schemas?.NotificationRuleCreateBodyData ?? {};
  const notificationRuleUpdate =
    document.components?.schemas?.NotificationRuleUpdateBodyData ?? {};
  if (
    notificationRuleCreate.properties?.targets?.minItems !== 1 ||
    notificationRuleUpdate.properties?.targets?.minItems !== 1 ||
    notificationRuleCreate.properties?.window_seconds?.minimum !== 1 ||
    notificationRuleCreate.properties?.window_seconds?.maximum !== 86_400 ||
    notificationRuleCreate.properties?.threshold_count?.maximum !== 9_999 ||
    notificationRuleCreate.properties?.cooldown_seconds?.minimum !== 0 ||
    (notificationRuleUpdate.required ?? []).length !== 0
  ) {
    throw new Error("notification rule bounds are out of sync");
  }
  const notificationDeliveryPolicy =
    document.components?.schemas?.NotificationDeliveryPolicyData ?? {};
  if (
    notificationDeliveryPolicy.properties?.timeout_seconds?.maximum !== 30 ||
    notificationDeliveryPolicy.properties?.max_attempts?.maximum !== 10 ||
    notificationDeliveryPolicy.properties?.backoff_seconds?.minimum !== 5 ||
    notificationDeliveryPolicy.properties?.backoff_seconds?.maximum !== 3_600
  ) {
    throw new Error("notification delivery retry policy is out of sync");
  }
  const notificationDeliveryParameters =
    document.paths?.["/api/admin/notifications/deliveries"]?.get?.parameters ??
    [];
  const notificationDeliveryLimit = notificationDeliveryParameters.find(
    (parameter) => parameter.name === "limit",
  )?.schema;
  const notificationDeliveryStatus = notificationDeliveryParameters.find(
    (parameter) => parameter.name === "status",
  )?.schema;
  if (
    notificationDeliveryLimit?.default !== 20 ||
    notificationDeliveryLimit?.oneOf?.[0]?.minimum !== 1 ||
    JSON.stringify(notificationDeliveryStatus?.enum) !==
      JSON.stringify([
        "queued",
        "sending",
        "success",
        "failed",
        "gave_up",
        "skipped",
      ])
  ) {
    throw new Error("notification delivery query contract is out of sync");
  }
  const availability =
    document.components?.schemas?.ProtocolMappingFeatureData ?? {};
  if (!(availability.required ?? []).includes("availability")) {
    throw new Error("protocol mapping must always emit nullable availability");
  }
  const promptUpdate =
    document.components?.schemas?.RunModePromptPreferencesUpdateData ?? {};
  if ((promptUpdate.required ?? []).length !== 0) {
    throw new Error("run-mode prompt preferences must remain partial");
  }
  const smartRuntimeRequired =
    document.components?.schemas?.SmartConnectRuntimeData?.required ?? [];
  for (const field of ["last_sync_at", "last_sync_error"]) {
    if (!smartRuntimeRequired.includes(field)) {
      throw new Error(
        `smart connect runtime must always emit nullable ${field}`,
      );
    }
  }
  const smartIpRequired =
    document.components?.schemas?.SmartConnectLocalIpData?.required ?? [];
  if (
    !smartIpRequired.includes("netmask") ||
    !smartIpRequired.includes("prefix")
  ) {
    throw new Error("smart connect local IP network metadata is incomplete");
  }
  const shareTimeout =
    document.components?.schemas?.FnosShareBypassData?.properties
      ?.upstream_timeout_ms;
  if (shareTimeout?.minimum !== 500 || shareTimeout?.maximum !== 15_000) {
    throw new Error("FNOS share bypass timeout bounds are out of sync");
  }
  const portIconUpdate =
    document.components?.schemas?.FnosPortIconHijackUpdateData?.properties ??
    {};
  if (portIconUpdate.updated_at) {
    throw new Error("FNOS port icon updated_at must remain server-owned");
  }
  const tuningReasonCodes =
    document.components?.schemas?.FnosNetworkTuningData?.properties
      ?.blocked_reason_code?.enum;
  if (
    JSON.stringify(tuningReasonCodes) !==
    JSON.stringify(["lite", "deployment", "platform", "permission"])
  ) {
    throw new Error("FNOS network tuning reason codes are out of sync");
  }
  const connectWafRuntimeRequired =
    document.components?.schemas?.FnosConnectWafRuntimeData?.required ?? [];
  for (const field of [
    "detected_http_port",
    "listener_port",
    "local_networks",
    "source",
    "last_sync_at",
    "last_error",
  ]) {
    if (!connectWafRuntimeRequired.includes(field)) {
      throw new Error(
        `FN Connect WAF runtime must always emit nullable ${field}`,
      );
    }
  }
  const certificateRuntimeRequired =
    document.components?.schemas?.FnosCertificateSyncRuntimeData?.required ??
    [];
  for (const field of [
    "last_sync_at",
    "last_result",
    "last_error",
    "failed_target_ids",
  ]) {
    if (!certificateRuntimeRequired.includes(field)) {
      throw new Error(
        `FNOS certificate sync runtime must always emit ${field}`,
      );
    }
  }
  const certificateItemRequired =
    document.components?.schemas?.FnosCertificateSyncItemData?.required ?? [];
  for (const field of [
    "valid_from",
    "valid_to",
    "fingerprint",
    "reason",
    "local",
  ]) {
    if (!certificateItemRequired.includes(field)) {
      throw new Error(
        `FNOS certificate sync item must always emit nullable ${field}`,
      );
    }
  }
  const certificateSyncBody =
    document.components?.schemas?.FnosCertificateSyncBodyData ?? {};
  if ((certificateSyncBody.required ?? []).includes("target_ids")) {
    throw new Error("FNOS certificate sync target_ids must remain optional");
  }
  for (const schemaName of ["StreamMappingData", "StreamMappingInputData"]) {
    const streamMapping = document.components?.schemas?.[schemaName] ?? {};
    if (
      streamMapping.properties?.listen_port?.minimum !== 1 ||
      streamMapping.properties?.listen_port?.maximum !== 65_535
    ) {
      throw new Error(`${schemaName} listen_port bounds are out of sync`);
    }
  }
  const subdomainData = document.components?.schemas?.SubdomainModeData ?? {};
  for (const field of [
    "public_http_port",
    "public_https_port",
    "passkey_rp_id",
  ]) {
    if (!(subdomainData.required ?? []).includes(field)) {
      throw new Error(`subdomain mode must always emit ${field}`);
    }
  }
  const subdomainResponseRequired =
    document.components?.schemas?.SubdomainModeResponseData?.required ?? [];
  if (!subdomainResponseRequired.includes("ssl_auto_selection")) {
    throw new Error(
      "subdomain mode writes must always emit nullable ssl_auto_selection",
    );
  }
  const basicAuthPassword =
    document.components?.schemas?.HostMappingBasicAuthInputData?.properties
      ?.password;
  if (basicAuthPassword?.writeOnly !== true) {
    throw new Error("host mapping Basic Auth password must remain write-only");
  }
  const basicAuthProbeRequired =
    document.components?.schemas?.HostMappingBasicAuthProbeData?.required ?? [];
  if (!basicAuthProbeRequired.includes("httpStatus")) {
    throw new Error("Basic Auth probe must always emit nullable httpStatus");
  }
  const advancedInput =
    document.components?.schemas?.AdvancedAuthConfigInputData?.properties ?? {};
  if (
    advancedInput.idle_ttl_seconds?.minimum !== 300 ||
    advancedInput.idle_ttl_seconds?.maximum !== 2_592_000 ||
    advancedInput.max_lifetime_seconds?.maximum !== 31_536_000 ||
    advancedInput.groups?.maxItems !== 16
  ) {
    throw new Error("advanced authentication limits are out of sync");
  }
  const advancedDetailsRequired =
    document.components?.schemas?.AdvancedAuthDetailsData?.required ?? [];
  if (!advancedDetailsRequired.includes("revision")) {
    throw new Error("advanced authentication revision must always be present");
  }
  const bookmarkContent =
    document.paths?.["/api/admin/config/host_mappings/bookmarks/export"]?.get
      ?.responses?.["200"]?.content;
  if (!bookmarkContent?.["text/html"] || bookmarkContent["application/json"]) {
    throw new Error("host mapping bookmarks must remain an HTML attachment");
  }
  const scanSettings =
    document.components?.schemas?.ScanDiscoverySettingsData?.properties ?? {};
  if (
    !scanSettings.intensityMode?.enum?.includes("auto") ||
    scanSettings.effectiveConcurrency?.minimum !== 1
  ) {
    throw new Error("scan discovery intensity contract is out of sync");
  }
  const scanJobBody =
    document.components?.schemas?.ScanDiscoverJobBodyData?.properties
      ?.target_cidrs;
  if (scanJobBody?.minItems !== 1 || scanJobBody?.maxItems !== 16) {
    throw new Error("scan discovery CIDR limits are out of sync");
  }
  const scanMetaRequired =
    document.components?.schemas?.ScanDiscoverMetaData?.required ?? [];
  const scanResultRequired =
    document.components?.schemas?.ScanDiscoverResultData?.required ?? [];
  if (
    scanMetaRequired.includes("services") ||
    !scanMetaRequired.includes("portRange") ||
    !scanResultRequired.includes("services")
  ) {
    throw new Error(
      "scan discovery meta and result boundaries are out of sync",
    );
  }
  const scanJobRequired =
    document.components?.schemas?.ScanDiscoverJobData?.required ?? [];
  for (const field of ["meta", "progress", "result", "error"]) {
    if (!scanJobRequired.includes(field)) {
      throw new Error(`scan discovery jobs must always emit nullable ${field}`);
    }
  }
  const scanJobParameters =
    document.paths?.["/api/admin/scan/discover/jobs/{job_id}"]?.get
      ?.parameters ?? [];
  if (
    !scanJobParameters.some(
      (parameter) =>
        parameter.name === "cursor" && parameter.schema?.minimum === 0,
    )
  ) {
    throw new Error("scan discovery cursor must remain non-negative");
  }
  const deepMonitorStart =
    document.components?.schemas?.DeepMonitorStartBodyData ?? {};
  const deepMonitorDuration = deepMonitorStart.properties?.duration_seconds;
  const deepMonitorExtendDuration =
    document.components?.schemas?.DeepMonitorExtendBodyData?.properties
      ?.duration_seconds;
  if (
    !deepMonitorDuration?.oneOf?.some((schema) => schema.const === 0) ||
    !deepMonitorDuration?.oneOf?.some(
      (schema) => schema.minimum === 300 && schema.maximum === 7_200,
    ) ||
    deepMonitorExtendDuration?.minimum !== 300 ||
    deepMonitorExtendDuration?.maximum !== 7_200 ||
    (deepMonitorStart.required ?? []).includes("duration_seconds")
  ) {
    throw new Error("deep monitor start duration contract is out of sync");
  }
  const deepMonitorEventRequired =
    document.components?.schemas?.DeepMonitorEventData?.required ?? [];
  for (const field of ["summary", "timing", "websocket_frame"]) {
    if (!deepMonitorEventRequired.includes(field)) {
      throw new Error(`deep monitor events must always emit nullable ${field}`);
    }
  }
  const deepMonitorEventsParameters =
    document.paths?.["/api/admin/deep-monitor/sessions/{session_id}/events"]
      ?.get?.parameters ?? [];
  const deepMonitorLimit = deepMonitorEventsParameters.find(
    (parameter) => parameter.name === "limit",
  );
  if (
    deepMonitorLimit?.schema?.minimum !== 1 ||
    deepMonitorLimit?.schema?.maximum !== 200
  ) {
    throw new Error("deep monitor event page limits are out of sync");
  }
  const deepMonitorPayload =
    document.paths?.[
      "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload"
    ]?.get;
  if (
    !deepMonitorPayload?.parameters?.some(
      (parameter) => parameter.name === "part" && parameter.required === true,
    ) ||
    !deepMonitorPayload.responses?.["200"]?.content?.[
      "application/octet-stream"
    ] ||
    !deepMonitorPayload.responses?.["204"]
  ) {
    throw new Error("deep monitor payload stream contract is out of sync");
  }
  const deepMonitorLive =
    document.paths?.["/api/admin/deep-monitor/sessions/{session_id}/live"]?.get;
  const deepMonitorDownload =
    document.paths?.["/api/admin/deep-monitor/sessions/{session_id}/download"]
      ?.get;
  if (
    !deepMonitorLive?.responses?.["200"]?.content?.["text/event-stream"] ||
    !deepMonitorDownload?.responses?.["200"]?.content?.["application/zip"] ||
    !deepMonitorDownload?.responses?.["204"]
  ) {
    throw new Error("deep monitor streaming media types are out of sync");
  }
  const dashboardDisplayRequest =
    document.paths?.["/api/admin/config/dashboard_display"]?.post?.requestBody
      ?.content?.["application/json"]?.schema?.$ref;
  if (
    dashboardDisplayRequest !==
    "#/components/schemas/DashboardDisplayUpdateData"
  ) {
    throw new Error(
      "dashboard display writes must use the partial input schema",
    );
  }
  const dashboardStatsParameters =
    document.paths?.["/api/admin/dashboard/stats"]?.get?.parameters ?? [];
  const rangeSec = dashboardStatsParameters.find(
    (parameter) => parameter.name === "rangeSec",
  );
  if (
    rangeSec?.schema?.minimum !== 60 ||
    rangeSec?.schema?.maximum !== 2_592_000
  ) {
    throw new Error("dashboard stats range must preserve its clamp boundaries");
  }
  const activeIpParameters =
    document.paths?.["/api/admin/dashboard/active-ips"]?.get?.parameters ?? [];
  if (
    !activeIpParameters.some(
      (parameter) => parameter.name === "host" && parameter.required === true,
    )
  ) {
    throw new Error(
      "dashboard active IPs must require the host query parameter",
    );
  }
  for (const [schema, fields] of [
    ["DashboardRealtimeData", ["by_host", "timestamp"]],
    ["DashboardHostTrafficData", ["active_ip_count"]],
    ["DashboardActiveIpsData", ["timestamp"]],
  ]) {
    const required = document.components?.schemas?.[schema]?.required ?? [];
    for (const field of fields) {
      if (!required.includes(field)) {
        throw new Error(`${schema} must always emit ${field}`);
      }
    }
  }
  const downloadStatuses =
    document.components?.schemas?.UpdateDownloadData?.properties?.status?.enum;
  if (
    JSON.stringify(downloadStatuses) !==
    JSON.stringify([
      "idle",
      "downloading",
      "verifying",
      "downloaded",
      "installing",
      "error",
    ])
  ) {
    throw new Error("update download statuses are out of sync");
  }
  const updateStatusRequired =
    document.components?.schemas?.UpdateStatusData?.required ?? [];
  if (!updateStatusRequired.includes("latest")) {
    throw new Error("update status must always emit nullable latest metadata");
  }
  const updateConfirmData =
    document.paths?.["/api/admin/update/confirm"]?.get?.responses?.["200"]
      ?.content?.["application/json"]?.schema?.properties?.data;
  if (
    !updateConfirmData?.anyOf?.some((schema) => schema.type === "null") ||
    !updateConfirmData?.anyOf?.some(
      (schema) => schema.$ref === "#/components/schemas/UpdateConfirmData",
    )
  ) {
    throw new Error(
      "update confirmation data must remain required and nullable",
    );
  }
  const ldapTestRequest =
    document.paths?.["/api/admin/auth/ldap/providers/{id}/test"]?.post
      ?.requestBody;
  if (!ldapTestRequest || ldapTestRequest.required !== false) {
    throw new Error("LDAP provider test request body must remain optional");
  }
  const wolDiscoveryParameters =
    document.paths?.["/api/admin/wol/discover/jobs/{id}"]?.get?.parameters ??
    [];
  if (
    !wolDiscoveryParameters.some(
      (parameter) => parameter.name === "cursor" && parameter.in === "query",
    )
  ) {
    throw new Error(
      "WOL discovery polling must declare its cursor query parameter",
    );
  }
  for (const [schema, property] of [
    ["WolLocalRelayInputData", "psk"],
    ["WolLocalRelayPairBodyData", "pairingCode"],
    ["WolBlinkerIntegrationInputData", "deviceKey"],
    ["WolBemfaIntegrationInputData", "privateKey"],
  ]) {
    if (
      document.components?.schemas?.[schema]?.properties?.[property]
        ?.writeOnly !== true
    ) {
      throw new Error(`${schema}.${property} must remain write-only`);
    }
  }
  if (
    !document.components?.schemas?.GatewayVisibilitySummaryData?.properties
      ?.range_count
  ) {
    throw new Error("gateway visibility summary must declare range_count");
  }
  const panelDeploymentTargets =
    document.components?.schemas?.PanelBootstrapData?.properties
      ?.deployment_target?.enum;
  if (
    JSON.stringify(panelDeploymentTargets) !==
    JSON.stringify([
      "fpk",
      "fpk-lite",
      "docker",
      "openwrt",
      "linux",
      "macos",
      "synology",
      "windows",
      "dev",
    ])
  ) {
    throw new Error("panel deployment targets are out of sync");
  }
  for (const schema of ["PanelPasswordBodyData", "PanelLoginBodyData"]) {
    if (
      document.components?.schemas?.[schema]?.properties?.password
        ?.writeOnly !== true
    ) {
      throw new Error(`${schema}.password must remain write-only`);
    }
  }
  const panelLogin429 =
    document.paths?.["/api/admin/panel/login"]?.post?.responses?.["429"];
  if (
    panelLogin429?.content?.["application/json"]?.schema?.$ref !==
    "#/components/schemas/PanelLoginRateLimitErrorData"
  ) {
    throw new Error(
      "panel login must document its extended 429 error envelope",
    );
  }
  const gatewayOperators =
    document.components?.schemas?.GatewayVisibilitySelectionInputData
      ?.properties?.operator?.enum;
  if (
    JSON.stringify(gatewayOperators) !==
    JSON.stringify(["电信", "联通", "移动"])
  ) {
    throw new Error("gateway visibility operators are out of sync");
  }
  const gatewayUpdateProperties =
    document.components?.schemas?.GatewaySettingsUpdateData?.properties ?? {};
  for (const derivedProperty of [
    "visibility",
    "proxy_headers",
    "host_response",
  ]) {
    if (derivedProperty in gatewayUpdateProperties) {
      throw new Error(
        `gateway settings update must not accept derived ${derivedProperty}`,
      );
    }
  }
  const gatewayPortalVersions =
    document.components?.schemas?.GatewayPortalUpdateData?.properties?.version
      ?.enum;
  if (JSON.stringify(gatewayPortalVersions) !== JSON.stringify(["v1", "v2"])) {
    throw new Error("gateway portal versions are out of sync");
  }
  const gatewayLogParameters =
    document.paths?.["/api/admin/gateway-logs/entries"]?.get?.parameters ?? [];
  for (const parameterName of ["pagination", "cursor", "waf_status"]) {
    if (
      !gatewayLogParameters.some(
        (parameter) => parameter.name === parameterName,
      )
    ) {
      throw new Error(
        `gateway log entries must declare ${parameterName} query parameter`,
      );
    }
  }
  const gatewayLogEntryProperties =
    document.components?.schemas?.GatewayLogEntryData?.properties ?? {};
  for (const property of ["auth_rule_group_id", "auth_grant_state"]) {
    if (!(property in gatewayLogEntryProperties)) {
      throw new Error(`gateway log entries must preserve ${property}`);
    }
  }
  if (
    "clients" in
    (document.components?.schemas?.GatewayLogAnalyticsData?.properties ?? {})
  ) {
    throw new Error("gateway log analytics must not expose raw client IPs");
  }
  const runtimeLogParameters =
    document.paths?.["/api/admin/runtime-health/logs/{component}"]?.get
      ?.parameters ?? [];
  const runtimeComponentParameter = runtimeLogParameters.find(
    (parameter) => parameter.name === "component" && parameter.in === "path",
  );
  if (
    JSON.stringify(runtimeComponentParameter?.schema?.enum) !==
    JSON.stringify(["management", "gateway_process"])
  ) {
    throw new Error("runtime log component path parameter is out of sync");
  }
  const runtimeLimitParameter = runtimeLogParameters.find(
    (parameter) => parameter.name === "limit" && parameter.in === "query",
  );
  if (
    runtimeLimitParameter?.schema?.minimum !== 1 ||
    runtimeLimitParameter?.schema?.maximum !== 500
  ) {
    throw new Error("runtime log limit must preserve the 1..500 boundary");
  }
  const runtimeArchiveContent =
    document.paths?.["/api/admin/runtime-health/diagnostics/archive"]?.get
      ?.responses?.["200"]?.content ?? {};
  if (
    runtimeArchiveContent["application/zip"]?.schema?.format !== "binary" ||
    runtimeArchiveContent["application/json"]
  ) {
    throw new Error("runtime diagnostics archive must remain a ZIP response");
  }
  const runtimeDiagnosticsProperties =
    document.components?.schemas?.RuntimeDiagnosticsData?.properties ?? {};
  if (!("collection" in runtimeDiagnosticsProperties)) {
    throw new Error(
      "runtime diagnostics must document its collection boundary",
    );
  }
  const runtimeComponentRequired =
    document.components?.schemas?.RuntimeComponentHealthData?.required ?? [];
  for (const nullableField of [
    "version",
    "commit",
    "pid",
    "instance_id",
    "started_at",
    "last_checked_at",
    "last_success_at",
    "reason_code",
  ]) {
    if (!runtimeComponentRequired.includes(nullableField)) {
      throw new Error(
        `runtime component health must always emit nullable ${nullableField}`,
      );
    }
  }
  const cidrCitiesParameters =
    document.paths?.["/api/admin/cidr/cities"]?.get?.parameters ?? [];
  const provinceParameter = cidrCitiesParameters.find(
    (parameter) => parameter.name === "province" && parameter.in === "query",
  );
  if (provinceParameter?.required !== true) {
    throw new Error("CIDR cities must require the province query parameter");
  }
  const cidrLookupParameters =
    document.paths?.["/api/admin/cidr/cidrs"]?.get?.parameters ?? [];
  const cidrOperatorParameter = cidrLookupParameters.find(
    (parameter) => parameter.name === "operator",
  );
  if (
    JSON.stringify(cidrOperatorParameter?.schema?.enum) !==
    JSON.stringify(["电信", "联通", "移动"])
  ) {
    throw new Error("CIDR lookup operator query is out of sync");
  }
  const ipLocationBatchItems =
    document.components?.schemas?.IpLocationBatchBodyData?.properties?.ips;
  if (ipLocationBatchItems?.maxItems !== 20) {
    throw new Error("IP location batch must preserve its 20-item limit");
  }
  const ipLocationSnapshotProperties =
    document.components?.schemas?.IpLocationSnapshotData?.properties ?? {};
  if (!("result" in ipLocationSnapshotProperties)) {
    throw new Error("IP location snapshots must preserve structured results");
  }
  const locationModes =
    document.components?.schemas?.IpLocationApiConfigData?.properties
      ?.ip_lookup_mode?.enum;
  if (JSON.stringify(locationModes) !== JSON.stringify(["online", "custom"])) {
    throw new Error("IP location API modes are out of sync");
  }
  for (const route of [
    "/api/admin/config/ip_location_api/test-ip-lookup",
    "/api/admin/config/ip_location_api/test-cidr",
  ]) {
    const responseSchema =
      document.paths?.[route]?.post?.responses?.["200"]?.content?.[
        "application/json"
      ]?.schema;
    if (!responseSchema?.$ref || responseSchema?.properties?.data) {
      throw new Error(`${route} must remain a direct JSON test response`);
    }
  }
  const scannerSettingsProperties =
    document.components?.schemas?.ScannerSettingsData?.properties ?? {};
  for (const runtimeProperty of [
    "cidrExemptionPolicyId",
    "cidrExemptionSourceCidrCount",
    "cidrExemptionRangeCount",
  ]) {
    if (!(runtimeProperty in scannerSettingsProperties)) {
      throw new Error(`scanner settings must preserve ${runtimeProperty}`);
    }
  }
  const scannerUpdateProperties =
    document.components?.schemas?.ScannerSettingsUpdateData?.properties ?? {};
  for (const readOnlyProperty of [
    "windowSeconds",
    "cidrExemptionPolicyId",
    "cidrExemptionSourceCidrCount",
    "cidrExemptionRangeCount",
  ]) {
    if (readOnlyProperty in scannerUpdateProperties) {
      throw new Error(
        `scanner settings update must not accept read-only ${readOnlyProperty}`,
      );
    }
  }
  const scannerDeleteSchema =
    document.paths?.["/api/admin/scanner/blacklist"]?.delete?.requestBody
      ?.content?.["application/json"]?.schema?.$ref;
  if (scannerDeleteSchema !== "#/components/schemas/IpListBodyData") {
    throw new Error("scanner blacklist deletion must preserve its JSON body");
  }
  const generalRecordProperties =
    document.components?.schemas?.GeneralBlacklistRecordData?.properties ?? {};
  for (const field of ["source", "comment", "created_at", "updated_at"]) {
    if (!(field in generalRecordProperties)) {
      throw new Error(`general blacklist records must preserve ${field}`);
    }
  }
  const overviewPointSchema =
    document.components?.schemas?.SecurityOverviewSeriesData?.properties
      ?.failedLogins?.items;
  if (
    overviewPointSchema?.items !== false ||
    overviewPointSchema?.prefixItems?.length !== 2
  ) {
    throw new Error("security overview series points must remain exact pairs");
  }
  const sshSummaryProperties =
    document.components?.schemas?.SshSecuritySummaryData?.properties ?? {};
  if (!("allowed_range_count" in sshSummaryProperties)) {
    throw new Error("SSH summary must preserve allowed_range_count");
  }
  const sshUpdateProperties =
    document.components?.schemas?.SshSecurityConfigUpdateData?.properties ?? {};
  for (const readOnlyProperty of ["configured_at", "updated_at"]) {
    if (readOnlyProperty in sshUpdateProperties) {
      throw new Error(
        `SSH config update must not accept read-only ${readOnlyProperty}`,
      );
    }
  }
  const sshBlocksDeleteSchema =
    document.paths?.["/api/admin/ssh-security/blocks"]?.delete?.requestBody
      ?.content?.["application/json"]?.schema?.$ref;
  if (
    sshBlocksDeleteSchema !== "#/components/schemas/SshBlocksDeleteBodyData"
  ) {
    throw new Error("SSH bulk block deletion must preserve its JSON body");
  }
  const sshLoginParameters =
    document.paths?.["/api/admin/ssh-security/login-logs"]?.get?.parameters ??
    [];
  const sshOutcomeParameter = sshLoginParameters.find(
    (parameter) => parameter.name === "outcome",
  );
  if (
    JSON.stringify(sshOutcomeParameter?.schema?.enum) !==
    JSON.stringify(["success", "failure"])
  ) {
    throw new Error("SSH login log outcome filter is out of sync");
  }
  const whitelistStatusEnum =
    document.components?.schemas?.WhitelistRecordData?.properties?.status
      ?.enum ?? [];
  if (!whitelistStatusEnum.includes("pending")) {
    throw new Error("whitelist record status must include pending grants");
  }
  const whitelistRefreshSchema =
    document.paths?.["/api/admin/whitelist/{id}/refresh"]?.post?.responses?.[
      "200"
    ]?.content?.["application/json"]?.schema?.$ref;
  if (
    whitelistRefreshSchema !==
    "#/components/schemas/WhitelistRefreshEnvelopeData"
  ) {
    throw new Error(
      "whitelist refresh must preserve its success-or-failure data envelope",
    );
  }
  const loggingUpdateProperties =
    document.components?.schemas?.GatewayLoggingConfigUpdateData?.properties ??
    {};
  for (const runtimeProperty of [
    "logs_dir",
    "dropped_entries",
    "queue_size",
    "queue_depth",
  ]) {
    if (runtimeProperty in loggingUpdateProperties) {
      throw new Error(
        `gateway logging update must not accept runtime ${runtimeProperty}`,
      );
    }
  }
  console.log(
    `[api-contract] validated ${operations.length} path/method operations (${typedDomainOperationsSeen.size} typed core operations)`,
  );
}

const temporaryDirectory =
  mode === "check"
    ? mkdtempSync(path.join(os.tmpdir(), "fn-knock-api-contract-"))
    : null;
try {
  const openapiPath =
    mode === "generate"
      ? path.join(root, "packages/api-contract/openapi.json")
      : path.join(temporaryDirectory, "openapi.json");
  const typesPath =
    mode === "generate"
      ? path.join(root, "packages/api-contract/src/schema.d.ts")
      : path.join(temporaryDirectory, "schema.d.ts");

  run("cargo", [
    "run",
    "--locked",
    "--manifest-path",
    "apps/server-admin-rs/Cargo.toml",
    "--bin",
    "export-openapi",
    "--",
    openapiPath,
  ]);
  validateContract(openapiPath);
  const executable = path.join(
    root,
    "node_modules/.bin",
    process.platform === "win32"
      ? "openapi-typescript.cmd"
      : "openapi-typescript",
  );
  run(executable, [openapiPath, "--output", typesPath]);

  if (mode === "check") {
    for (const [generated, committed] of [
      [openapiPath, path.join(root, "packages/api-contract/openapi.json")],
      [typesPath, path.join(root, "packages/api-contract/src/schema.d.ts")],
    ]) {
      if (readFileSync(generated, "utf8") !== readFileSync(committed, "utf8")) {
        throw new Error(
          `${path.relative(root, committed)} is stale; run npm run api:generate`,
        );
      }
    }
    console.log(
      "[api-contract] checked-in OpenAPI and TypeScript types are current",
    );
  }
} finally {
  if (temporaryDirectory)
    rmSync(temporaryDirectory, { recursive: true, force: true });
}
