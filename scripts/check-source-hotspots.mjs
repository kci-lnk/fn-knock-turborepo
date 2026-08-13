#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const budgets = [
  {
    path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization.rs",
    maxLines: 4_350,
  },
  {
    path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization/api.rs",
    maxLines: 575,
  },
  {
    path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization/scheduler.rs",
    maxLines: 500,
  },
  {
    path: "apps/server-admin-view/src/lib/api/config.ts",
    maxLines: 340,
  },
  {
    path: "apps/server-admin-view/src/lib/api/config-core-api.ts",
    maxLines: 210,
  },
  {
    path: "apps/server-admin-view/src/lib/api/config-proxy-api.ts",
    maxLines: 430,
  },
  {
    path: "apps/server-admin-view/src/lib/api/config-auth-api.ts",
    maxLines: 350,
  },
  {
    path: "apps/server-admin-view/src/lib/api/config-revisions.ts",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/lib/api/system.ts",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/lib/pollingLifecycle.ts",
    maxLines: 50,
  },
  {
    path: "apps/server-admin-view/src/store/config.ts",
    maxLines: 550,
  },
  {
    path: "apps/server-admin-view/src/store/systemClock.ts",
    maxLines: 190,
  },
  {
    path: "apps/server-admin-view/src/store/update.ts",
    maxLines: 310,
  },
  {
    path: "apps/server-admin-view/src/store/hostMappingMetadata.ts",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/store/useConfigRuntimeCapabilities.ts",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/useSystemEventDisplay.ts",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/systemEventDetailFields.ts",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/systemEventDescription.ts",
    maxLines: 330,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/systemEventValueFormatters.ts",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/oidc-provider-settings/LDAPProviderSettingsCard.vue",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/oidc-provider-settings/LDAPProviderEditorDialog.vue",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/oidc-provider-settings/LDAPTestCredentialsDialog.vue",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/oidc-provider-settings/useLdapProviderManagement.ts",
    maxLines: 310,
  },
  {
    path: "apps/server-admin-view/src/types.ts",
    maxLines: 15,
  },
  {
    path: "apps/server-admin-view/src/types/core.ts",
    maxLines: 470,
  },
  {
    path: "apps/server-admin-view/src/types/app-config.ts",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/types/auth-session.ts",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/types/gateway.ts",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/Layout.vue",
    maxLines: 460,
  },
  {
    path: "apps/server-admin-view/src/views/DeepMonitor.vue",
    maxLines: 430,
  },
  {
    path: "apps/server-admin-view/src/views/waf-logs/useWafLogsResource.ts",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/ssl-settings/useAcmeJobPolling.ts",
    maxLines: 190,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/frp/useFrpTunnelController.ts",
    maxLines: 460,
  },
  {
    path: "apps/server-admin-view/src/views/GatewayRequestLogs.vue",
    maxLines: 270,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/GatewayRequestLogsActions.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/GatewayRequestLogsFilters.vue",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/useGatewayRequestLogsResource.ts",
    maxLines: 400,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/GatewayRequestLogsPagination.vue",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/components/CursorPaginationDock.vue",
    maxLines: 240,
  },
  {
    path: "apps/server-admin-view/src/components/cursor-pagination-contract.ts",
    maxLines: 20,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/GatewayRequestLogsTable.vue",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/GatewayRequestLogMobileRow.vue",
    maxLines: 250,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/GatewayRequestLogDesktopRow.vue",
    maxLines: 290,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/gateway-request-log-row-contract.ts",
    maxLines: 40,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/model.ts",
    maxLines: 10,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/gateway-request-log-types.ts",
    maxLines: 15,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/gatewayRequestLogFilters.ts",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/gatewayRequestLogPresentation.ts",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/gateway-request-logs/gatewayRequestLogDetails.ts",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/WAFLogs.vue",
    maxLines: 270,
  },
  {
    path: "apps/server-admin-view/src/views/waf-logs/WAFLogsHeader.vue",
    maxLines: 150,
  },
  {
    path: "apps/server-admin-view/src/views/waf-logs/WAFLogsFilters.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/waf-logs/WAFLogsPagination.vue",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingsCard.vue",
    maxLines: 150,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingsTable.vue",
    maxLines: 220,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingTableRow.vue",
    maxLines: 220,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingGroupHeaderRow.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-mappings-card-contract.ts",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingsCardHeader.vue",
    maxLines: 260,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingStatusIndicators.vue",
    maxLines: 70,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingStatusTooltip.vue",
    maxLines: 70,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingAvailabilityIndicators.vue",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingAccessIndicators.vue",
    maxLines: 150,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingSecurityIndicators.vue",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-mapping-status-contract.ts",
    maxLines: 50,
  },
  {
    path: "apps/server-admin-view/src/components/StaleHostMappingsCleanupDialog.vue",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/components/stale-host-mappings/StaleHostMappingsResults.vue",
    maxLines: 220,
  },
  {
    path: "apps/server-admin-view/src/components/stale-host-mappings/useStaleHostMappingsCleanupDialog.ts",
    maxLines: 190,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingsBatchActions.vue",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingTitleCell.vue",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingRowActions.vue",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingNotices.vue",
    maxLines: 65,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingDialog.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingBasicForm.vue",
    maxLines: 210,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingAdvancedSettings.vue",
    maxLines: 40,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingAccessSettings.vue",
    maxLines: 190,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingProxyProtocolSettings.vue",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingVisibilityEntry.vue",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-mapping-dialog-contract.ts",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/model.ts",
    maxLines: 10,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-model-types.ts",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-host-model.ts",
    maxLines: 330,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-mapping-model.ts",
    maxLines: 530,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/subdomain-collection-model.ts",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainAdvancedAuth.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainAdvancedAuthEditor.vue",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/useSubdomainAdvancedAuthPage.ts",
    maxLines: 240,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/AdvancedAuthRuleGroups.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/AdvancedAuthRuleGroupCard.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/AdvancedAuthConditionEditor.vue",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/advanced-auth-rule-contract.ts",
    maxLines: 40,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/AdvancedAuthDurationSettings.vue",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/WOLManagement.vue",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/WolTargetsTab.vue",
    maxLines: 240,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/WolRelaysTab.vue",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/WolManagementDialogs.vue",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolManagementPage.ts",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolResources.ts",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolLocalRelay.ts",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolPortalSettings.ts",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolRelayManagement.ts",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolTargetManagement.ts",
    maxLines: 210,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/useWolDiscovery.ts",
    maxLines: 160,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/WOLPortalSettingsDialog.vue",
    maxLines: 75,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/frp/FrpcInstancePage.vue",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/frp/useFrpcInstancePage.ts",
    maxLines: 340,
  },
  {
    path: "apps/server-admin-view/src/views/IPWhitelist.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/ip-whitelist/WhitelistRecordsPanel.vue",
    maxLines: 320,
  },
  {
    path: "apps/server-admin-view/src/views/ip-whitelist/WhitelistRegionGroups.vue",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/ip-whitelist/useIpWhitelistPage.ts",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/ip-whitelist/whitelistPresentation.ts",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/IpBlacklistTab.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/IpBlacklistOverview.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/IpBlacklistRecordsPanel.vue",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/IpBlacklistDetailDialog.vue",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/useIpBlacklistPage.ts",
    maxLines: 270,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/GeneralBlacklistTab.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/GeneralBlacklistRecordsPanel.vue",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/GeneralBlacklistAddDialog.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/session-management/useGeneralBlacklistPage.ts",
    maxLines: 260,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/FnosSettings.vue",
    maxLines: 320,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/fnos-settings/useFnosSettingsController.ts",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/model.ts",
    maxLines: 10,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/ddns-model-types.ts",
    maxLines: 160,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/ddns-config-model.ts",
    maxLines: 520,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/ddns-validation.ts",
    maxLines: 460,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetDialog.vue",
    maxLines: 160,
  },
  {
    path: "apps/server-admin-view/src/views/DDNSManagement.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSManagementContent.vue",
    maxLines: 380,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/useDDNSManagementPage.ts",
    maxLines: 650,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetBasicFields.vue",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetAddressFields.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetAddressBaseFields.vue",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetStaticAddressFields.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetInterfaceAddressFields.vue",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSTargetProviderFields.vue",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/ddns-target-dialog-contract.ts",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationOverview.vue",
    maxLines: 250,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationDomains.vue",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationTechnicalStatus.vue",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/useCloudflareOptimizationCardPresentation.ts",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationSourceSettings.vue",
    maxLines: 220,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationScanResults.vue",
    maxLines: 320,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareManagedTunnelCard.vue",
    maxLines: 310,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareReconcilePlan.vue",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/cloudflareManagedPresentation.ts",
    maxLines: 270,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/useCloudflaredRuntime.ts",
    maxLines: 430,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/useCloudflareManagedTunnel.ts",
    maxLines: 370,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/useCloudflareOptimization.ts",
    maxLines: 400,
  },
  {
    path: "apps/server-admin-view/src/composables/useScanIntensityMatrix.ts",
    maxLines: 500,
  },
  {
    path: "apps/server-admin-view/src/composables/scanIntensityShaders.ts",
    maxLines: 150,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/CaptchaSettings.vue",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/captcha/CaptchaConfigField.vue",
    maxLines: 40,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/captcha/PowCaptchaSettingsFields.vue",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/captcha/TurnstileCaptchaSettingsFields.vue",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/SmartConnectSettings.vue",
    maxLines: 310,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/smart-connect/SmartConnectFormPanel.vue",
    maxLines: 340,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/RuntimeTab.vue",
    maxLines: 350,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/useRuntimeHealth.ts",
    maxLines: 260,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/runtimePresentation.ts",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/RulesTab.vue",
    maxLines: 150,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/NotificationRuleEditorDialog.vue",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/NotificationRuleDialogHeader.vue",
    maxLines: 70,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/NotificationRuleEventTypes.vue",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/NotificationRuleConditions.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/NotificationRuleTargets.vue",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/notification-rule-editor-contract.ts",
    maxLines: 20,
  },
  {
    path: "apps/server-admin-view/src/views/event-center/notifications/NotificationRulesClearDialog.vue",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/GatewayLocationsSettings.vue",
    maxLines: 170,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-locations/GatewayLocationHostSummary.vue",
    maxLines: 110,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-locations/GatewayLocationRulesTable.vue",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-locations/useGatewayLocationsPage.ts",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/SessionSettings.vue",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/session-settings/SessionAccessPolicyPanel.vue",
    maxLines: 190,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/session-settings/useSessionSettingsController.ts",
    maxLines: 310,
  },
  {
    path: "apps/server-admin-view/src/views/AboutUpdate.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/about-update/AboutUpdateDeploymentNotices.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/about-update/AboutUpdateVersionPanel.vue",
    maxLines: 160,
  },
  {
    path: "apps/server-admin-view/src/views/about-update/AboutUpdateProgressOverlay.vue",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/about-update/useAboutUpdatePage.ts",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/AuthSettings.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/auth-settings/AuthSettingsHeader.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/auth-settings/AuthSettingsTables.vue",
    maxLines: 130,
  },
  {
    path: "apps/server-admin-view/src/views/auth-settings/AuthSettingsDialogs.vue",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/auth-settings/useAuthSettingsPage.ts",
    maxLines: 250,
  },
  {
    path: "apps/server-admin-view/src/views/ReverseProxy.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/reverse-proxy/ReverseProxyMappingsCard.vue",
    maxLines: 290,
  },
  {
    path: "apps/server-admin-view/src/views/reverse-proxy/ReverseProxyDialogs.vue",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/reverse-proxy/useReverseProxyPage.ts",
    maxLines: 320,
  },
  {
    path: "apps/server-admin-view/src/views/request-analysis/RequestAnalyticsTab.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/request-analysis/RequestAnalyticsActions.vue",
    maxLines: 70,
  },
  {
    path: "apps/server-admin-view/src/views/request-analysis/RequestAnalyticsOverview.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/request-analysis/RequestAnalyticsBreakdowns.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/request-analysis/useRequestAnalyticsPage.ts",
    maxLines: 350,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/GatewayPortalSettings.vue",
    maxLines: 100,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-portal/GatewayPortalSettingsPanel.vue",
    maxLines: 160,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-portal/GatewayPortalChoiceSetting.vue",
    maxLines: 70,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-portal/useGatewayPortalSettings.ts",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/ScannerFirewallSettings.vue",
    maxLines: 190,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/scanner-firewall/ScannerFirewallExemptions.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/scanner-firewall/ScannerFirewallThresholds.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/scanner-firewall/useScannerFirewallSettings.ts",
    maxLines: 220,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/GatewayHostToggleSettings.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-host-toggle/GatewayHostToggleTable.vue",
    maxLines: 180,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-host-toggle/gatewayHostToggleTypes.ts",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/gateway-host-toggle/useGatewayHostToggleSettings.ts",
    maxLines: 210,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSAddressSourceFields.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSAddressSourceBaseFields.vue",
    maxLines: 230,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSStaticAddressFields.vue",
    maxLines: 120,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/DDNSInterfaceAddressFields.vue",
    maxLines: 200,
  },
  {
    path: "apps/server-admin-view/src/views/ddns-management/ddns-address-source-fields-contract.ts",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/MaintenanceSettings.vue",
    maxLines: 50,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/MaintenanceBackupPanel.vue",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/MaintenanceBackupDialogs.vue",
    maxLines: 150,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/MaintenanceDangerZone.vue",
    maxLines: 160,
  },
  {
    path: "apps/server-admin-view/src/views/system-settings/maintenance-settings-contract.ts",
    maxLines: 20,
  },
  {
    path: "apps/server-admin-view/src/views/SSHSecurity.vue",
    maxLines: 40,
  },
  {
    path: "apps/server-admin-view/src/views/ssh-security/SSHSecurityActionsMenu.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/ssh-security/SSHSecurityFormFields.vue",
    maxLines: 290,
  },
  {
    path: "apps/server-admin-view/src/views/ssh-security/SSHSecurityConfigurationCard.vue",
    maxLines: 90,
  },
  {
    path: "apps/server-admin-view/src/views/ssh-security/SSHSecurityActivityTabs.vue",
    maxLines: 60,
  },
  {
    path: "apps/server-admin-view/src/views/ssh-security/SSHSecurityClearFirewallDialog.vue",
    maxLines: 80,
  },
  {
    path: "apps/server-admin-view/src/views/ssh-security/ssh-security-contract.ts",
    maxLines: 20,
  },
  {
    path: "apps/server-admin-view/src/views/SubdomainProxy.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainProxyOverview.vue",
    maxLines: 240,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainProxyDialogs.vue",
    maxLines: 280,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/useSubdomainProxyPage.ts",
    maxLines: 720,
  },
  {
    path: "apps/server-admin-view/src/views/WebTerminal.vue",
    maxLines: 30,
  },
  {
    path: "apps/server-admin-view/src/views/web-terminal/WebTerminalWorkspace.vue",
    maxLines: 260,
  },
  {
    path: "apps/server-admin-view/src/views/web-terminal/WebTerminalDialogs.vue",
    maxLines: 70,
  },
  {
    path: "apps/server-admin-view/src/views/web-terminal/useWebTerminalPage.ts",
    maxLines: 450,
  },
  {
    path: "apps/server-admin-view/src/views/ssl-settings/AcmeCert.vue",
    maxLines: 40,
  },
  {
    path: "apps/server-admin-view/src/views/ssl-settings/AcmeCertificateHeader.vue",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/ssl-settings/AcmeCertificateApplicationsTable.vue",
    maxLines: 360,
  },
  {
    path: "apps/server-admin-view/src/views/ssl-settings/AcmeCertificateWorkflowPanels.vue",
    maxLines: 140,
  },
  {
    path: "apps/server-admin-view/src/views/ssl-settings/acme-certificate-contract.ts",
    maxLines: 20,
  },
];

const countLines = (content) => {
  if (!content) return 0;
  const newlines = content.match(/\n/g)?.length ?? 0;
  return content.endsWith("\n") ? newlines : newlines + 1;
};

const failures = [];
for (const budget of budgets) {
  const absolutePath = path.join(root, budget.path);
  let lines;
  try {
    lines = countLines(readFileSync(absolutePath, "utf8"));
  } catch (error) {
    failures.push(
      `${budget.path} cannot be read (${error.code ?? error.message})`,
    );
    continue;
  }
  console.log(
    `[source-hotspot] ${budget.path}: ${lines}/${budget.maxLines} lines`,
  );
  if (lines > budget.maxLines) {
    failures.push(
      `${budget.path} has ${lines} lines (limit ${budget.maxLines})`,
    );
  }
}

if (failures.length > 0) {
  throw new Error(`[source-hotspot] ${failures.join("; ")}`);
}
