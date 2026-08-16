import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const lineCount = (source: string) => source.trimEnd().split(/\r?\n/u).length;

const pageBudgets = [
  ["../src/components/ScanDiscoveryIntensityDialog.vue", 620],
  ["../src/views/event-center/notifications/RulesTab.vue", 150],
  [
    "../src/views/event-center/notifications/NotificationRuleEditorDialog.vue",
    80,
  ],
  [
    "../src/views/event-center/notifications/NotificationRuleDialogHeader.vue",
    70,
  ],
  [
    "../src/views/event-center/notifications/NotificationRuleEventTypes.vue",
    100,
  ],
  [
    "../src/views/event-center/notifications/NotificationRuleConditions.vue",
    130,
  ],
  ["../src/views/event-center/notifications/NotificationRuleTargets.vue", 170],
  [
    "../src/views/event-center/notifications/NotificationRulesClearDialog.vue",
    80,
  ],
  ["../src/views/system-settings/RunModeSettings.vue", 460],
  ["../src/views/ssl-settings/AcmeCert.vue", 40],
  ["../src/views/ssl-settings/AcmeCertificateHeader.vue", 140],
  ["../src/views/ssl-settings/AcmeCertificateApplicationsTable.vue", 360],
  ["../src/views/ssl-settings/AcmeCertificateWorkflowPanels.vue", 140],
  ["../src/views/WAFLogs.vue", 270],
  ["../src/views/waf-logs/WAFLogsHeader.vue", 150],
  ["../src/views/waf-logs/WAFLogsFilters.vue", 90],
  ["../src/views/waf-logs/WAFLogsPagination.vue", 60],
  ["../src/components/CursorPaginationDock.vue", 240],
  ["../src/views/GatewayRequestLogs.vue", 280],
  ["../src/views/gateway-request-logs/GatewayRequestLogsFilters.vue", 230],
  ["../src/views/gateway-request-logs/GatewayRequestLogsPagination.vue", 60],
  ["../src/views/Dashboard.vue", 550],
  ["../src/views/DDNSManagement.vue", 30],
  ["../src/views/ddns-management/DDNSManagementContent.vue", 380],
  ["../src/views/ddns-management/DDNSTargetDialog.vue", 160],
  ["../src/views/ddns-management/DDNSTargetAddressFields.vue", 30],
  ["../src/views/ddns-management/DDNSTargetAddressBaseFields.vue", 230],
  ["../src/views/ddns-management/DDNSTargetStaticAddressFields.vue", 120],
  ["../src/views/ddns-management/DDNSTargetInterfaceAddressFields.vue", 200],
  ["../src/views/ddns-management/DDNSAddressSourceFields.vue", 30],
  ["../src/views/ddns-management/DDNSAddressSourceBaseFields.vue", 230],
  ["../src/views/ddns-management/DDNSStaticAddressFields.vue", 120],
  ["../src/views/ddns-management/DDNSInterfaceAddressFields.vue", 200],
  ["../src/views/AuthSettings.vue", 30],
  ["../src/views/auth-settings/AuthSettingsHeader.vue", 130],
  ["../src/views/auth-settings/AuthSettingsTables.vue", 130],
  ["../src/views/auth-settings/AuthSettingsDialogs.vue", 230],
  ["../../server-auth-view/src/views/Login.vue", 590],
  ["../src/views/Layout.vue", 460],
  ["../src/views/WebTerminal.vue", 30],
  ["../src/views/web-terminal/WebTerminalWorkspace.vue", 260],
  ["../src/views/web-terminal/WebTerminalDialogs.vue", 70],
  ["../src/views/system-settings/WAFSettings.vue", 410],
  ["../src/views/system-settings/FeaturesSettings.vue", 180],
  ["../src/components/charts/TimeSeriesChart.vue", 130],
  ["../src/views/system-settings/MaintenanceSettings.vue", 50],
  ["../src/views/system-settings/MaintenanceBackupPanel.vue", 300],
  ["../src/views/system-settings/MaintenanceBackupDialogs.vue", 150],
  ["../src/views/system-settings/MaintenanceDangerZone.vue", 160],
  ["../src/views/OIDCProviderSettings.vue", 360],
  ["../src/views/oidc-provider-settings/LDAPProviderSettingsCard.vue", 200],
  ["../src/views/oidc-provider-settings/LDAPProviderEditorDialog.vue", 230],
  ["../src/views/ssl-settings/AcmeApplicationDialog.vue", 360],
  ["../src/views/tunnel/FrpTunnel.vue", 450],
  ["../src/views/tunnel/frp/FrpcInstancePage.vue", 300],
  ["../src/views/tunnel/CloudflareTunnel.vue", 400],
  ["../src/views/ssl-settings/CertConfig.vue", 470],
  ["../src/views/SSHSecurity.vue", 40],
  ["../src/views/ssh-security/SSHSecurityActionsMenu.vue", 90],
  ["../src/views/ssh-security/SSHSecurityFormFields.vue", 290],
  ["../src/views/ssh-security/SSHSecurityConfigurationCard.vue", 90],
  ["../src/views/ssh-security/SSHSecurityActivityTabs.vue", 60],
  ["../src/views/ssh-security/SSHSecurityClearFirewallDialog.vue", 80],
  ["../src/views/system-settings/IpLocationSettings.vue", 420],
  ["../src/views/session-management/mobility/SessionMobilityPage.vue", 390],
  ["../src/views/ReverseProxy.vue", 30],
  ["../src/views/reverse-proxy/ReverseProxyMappingsCard.vue", 290],
  ["../src/views/reverse-proxy/ReverseProxyDialogs.vue", 80],
  ["../src/views/IPWhitelist.vue", 90],
  ["../src/views/ip-whitelist/WhitelistRecordsPanel.vue", 320],
  ["../src/views/ip-whitelist/WhitelistRegionGroups.vue", 140],
  ["../src/views/session-management/IpBlacklistTab.vue", 30],
  ["../src/views/session-management/IpBlacklistOverview.vue", 90],
  ["../src/views/session-management/IpBlacklistRecordsPanel.vue", 280],
  ["../src/views/session-management/IpBlacklistDetailDialog.vue", 110],
  ["../src/views/session-management/GeneralBlacklistTab.vue", 30],
  ["../src/views/session-management/GeneralBlacklistRecordsPanel.vue", 280],
  ["../src/views/session-management/GeneralBlacklistAddDialog.vue", 120],
  ["../src/views/SubdomainProxy.vue", 30],
  ["../src/views/subdomain-proxy/SubdomainProxyOverview.vue", 240],
  ["../src/views/subdomain-proxy/SubdomainProxyDialogs.vue", 280],
  ["../src/views/event-center/notifications/ProvidersTab.vue", 230],
  ["../src/views/system-settings/GatewayLocationsSettings.vue", 170],
  [
    "../src/views/system-settings/gateway-locations/GatewayLocationHostSummary.vue",
    110,
  ],
  [
    "../src/views/system-settings/gateway-locations/GatewayLocationRulesTable.vue",
    200,
  ],
  ["../src/views/StreamMappings.vue", 390],
  ["../src/components/HostTrafficActivity.vue", 420],
  ["../src/views/system-settings/SessionSettings.vue", 180],
  [
    "../src/views/system-settings/session-settings/SessionAccessPolicyPanel.vue",
    190,
  ],
  ["../src/views/system-settings/SmartConnectSettings.vue", 310],
  ["../src/views/system-settings/smart-connect/SmartConnectFormPanel.vue", 340],
  ["../src/views/system-settings/CaptchaSettings.vue", 300],
  ["../src/views/system-settings/captcha/PowCaptchaSettingsFields.vue", 180],
  [
    "../src/views/system-settings/captcha/TurnstileCaptchaSettingsFields.vue",
    170,
  ],
  ["../src/views/PasskeySettings.vue", 360],
  ["../src/views/system-settings/FnosSettings.vue", 320],
  ["../src/views/WOLManagement.vue", 100],
  ["../src/views/wol-management/WolTargetsTab.vue", 240],
  ["../src/views/wol-management/WolRelaysTab.vue", 170],
  ["../src/views/wol-management/WolManagementDialogs.vue", 110],
  ["../src/views/subdomain-proxy/SubdomainMappingDialog.vue", 130],
  ["../src/views/subdomain-proxy/SubdomainMappingBasicForm.vue", 210],
  ["../src/views/subdomain-proxy/SubdomainMappingAdvancedSettings.vue", 40],
  ["../src/views/subdomain-proxy/SubdomainMappingAccessSettings.vue", 190],
  [
    "../src/views/subdomain-proxy/SubdomainMappingProxyProtocolSettings.vue",
    170,
  ],
  ["../src/views/subdomain-proxy/SubdomainMappingVisibilityEntry.vue", 110],
  ["../src/components/StaleHostMappingsCleanupDialog.vue", 180],
  ["../src/components/stale-host-mappings/StaleHostMappingsResults.vue", 220],
  ["../src/views/subdomain-proxy/SubdomainMappingStatusIndicators.vue", 70],
  ["../src/views/subdomain-proxy/SubdomainMappingStatusTooltip.vue", 70],
  [
    "../src/views/subdomain-proxy/SubdomainMappingAvailabilityIndicators.vue",
    100,
  ],
  ["../src/views/subdomain-proxy/SubdomainMappingAccessIndicators.vue", 150],
  ["../src/views/subdomain-proxy/SubdomainMappingSecurityIndicators.vue", 180],
  ["../src/views/subdomain-proxy/SubdomainMappingsCard.vue", 150],
  ["../src/views/subdomain-proxy/SubdomainMappingsTable.vue", 220],
  ["../src/views/subdomain-proxy/SubdomainMappingTableRow.vue", 220],
  ["../src/views/subdomain-proxy/SubdomainMappingGroupHeaderRow.vue", 130],
  ["../src/views/subdomain-proxy/AdvancedAuthRuleGroups.vue", 120],
  ["../src/views/subdomain-proxy/AdvancedAuthRuleGroupCard.vue", 120],
  ["../src/views/subdomain-proxy/AdvancedAuthConditionEditor.vue", 280],
  ["../src/views/subdomain-proxy/SubdomainAdvancedAuth.vue", 120],
  ["../src/views/subdomain-proxy/SubdomainAdvancedAuthEditor.vue", 140],
  ["../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue", 120],
  ["../src/views/tunnel/cloudflare/CloudflareOptimizationOverview.vue", 250],
  ["../src/views/tunnel/cloudflare/CloudflareOptimizationDomains.vue", 170],
  [
    "../src/views/tunnel/cloudflare/CloudflareOptimizationTechnicalStatus.vue",
    80,
  ],
  ["../src/views/ssl-settings/SelfSignedCA.vue", 380],
  ["../src/views/system-settings/GatewaySettings.vue", 280],
  ["../src/views/system-settings/GatewayHostToggleSettings.vue", 120],
  [
    "../src/views/system-settings/gateway-host-toggle/GatewayHostToggleTable.vue",
    180,
  ],
  ["../src/views/system-settings/ScannerFirewallSettings.vue", 190],
  [
    "../src/views/system-settings/scanner-firewall/ScannerFirewallExemptions.vue",
    120,
  ],
  [
    "../src/views/system-settings/scanner-firewall/ScannerFirewallThresholds.vue",
    120,
  ],
  ["../src/views/event-center/notifications/DeliveriesTab.vue", 330],
  ["../src/views/event-center/EventsTab.vue", 430],
  ["../src/views/event-center/RuntimeTab.vue", 350],
  ["../src/views/AboutUpdate.vue", 130],
  ["../src/views/about-update/AboutUpdateDeploymentNotices.vue", 120],
  ["../src/views/about-update/AboutUpdateVersionPanel.vue", 160],
  ["../src/views/about-update/AboutUpdateProgressOverlay.vue", 80],
  ["../src/views/request-analysis/RequestAnalyticsTab.vue", 90],
  ["../src/views/request-analysis/RequestAnalyticsActions.vue", 70],
  ["../src/views/request-analysis/RequestAnalyticsOverview.vue", 120],
  ["../src/views/request-analysis/RequestAnalyticsBreakdowns.vue", 90],
  ["../src/views/system-settings/GatewayPortalSettings.vue", 100],
  [
    "../src/views/system-settings/gateway-portal/GatewayPortalSettingsPanel.vue",
    160,
  ],
  [
    "../src/views/system-settings/gateway-portal/GatewayPortalChoiceSetting.vue",
    70,
  ],
] as const;

describe("large Vue architecture", () => {
  it("keeps refactored SFCs within their presentation budgets", () => {
    for (const [path, maximum] of pageBudgets) {
      const actual = lineCount(readSource(path));
      assert.ok(
        actual <= maximum,
        `${path} has ${actual} lines; expected at most ${maximum}`,
      );
    }
  });

  it("keeps gateway location validation and host selection out of the page", () => {
    const source = readSource(
      "../src/views/system-settings/GatewayLocationsSettings.vue",
    );
    const controllerSource = readSource(
      "../src/views/system-settings/gateway-locations/useGatewayLocationsPage.ts",
    );
    assert.match(source, /useGatewayLocationsPage/u);
    assert.match(source, /GatewayLocationHostPickerDialog/u);
    assert.match(source, /GatewayLocationHostSummary/u);
    assert.match(source, /GatewayLocationRulesTable/u);
    assert.doesNotMatch(source, /saveHostMappings|useAsyncAction/u);
    assert.match(controllerSource, /useGatewayLocationEditor/u);
    assert.doesNotMatch(source, /forbiddenResponseHeaders/u);
    assert.doesNotMatch(source, /cleanHostLocationPath/u);
  });

  it("keeps stream mapping form state inside its editor component", () => {
    const source = readSource("../src/views/StreamMappings.vue");
    const configApiSource = readSource("../src/lib/api/config.ts");
    const proxyApiSource = readSource("../src/lib/api/config-proxy-api.ts");
    const streamApiSource = readSource("../src/lib/api/config-stream-api.ts");
    const tableSource = readSource(
      "../src/views/stream-mappings/StreamMappingTable.vue",
    );
    const rowActionsSource = readSource(
      "../src/views/stream-mappings/StreamMappingRowActions.vue",
    );
    const editorSource = readSource(
      "../src/views/stream-mappings/StreamMappingEditorDialog.vue",
    );
    const serviceDialogSource = readSource(
      "../src/views/stream-mappings/StreamServiceProfileDialog.vue",
    );
    assert.match(source, /StreamMappingEditorDialog/u);
    assert.match(source, /StreamMappingDisabledAlert/u);
    assert.match(source, /StreamMappingTable/u);
    assert.match(tableSource, /InlineCommentEditor/u);
    assert.match(tableSource, /StreamMappingRowActions/u);
    assert.match(tableSource, /table-fixed/u);
    assert.doesNotMatch(tableSource, /ConfirmDangerPopover/u);
    assert.match(rowActionsSource, /rounded-r-none/u);
    assert.match(rowActionsSource, /DropdownMenu/u);
    assert.match(rowActionsSource, /variant="destructive"/u);
    assert.match(editorSource, /authRequiredEnabledHint/u);
    assert.match(editorSource, /authRequiredDisabledHint/u);
    assert.match(serviceDialogSource, /<option value="" disabled>/u);
    assert.match(serviceDialogSource, /clearService/u);
    assert.match(source, /streamMappingModel/u);
    assert.match(configApiSource, /configStreamApi/u);
    assert.match(streamApiSource, /updateStreamBypassPolicy/u);
    assert.doesNotMatch(proxyApiSource, /updateStreamBypassPolicy/u);
    assert.doesNotMatch(source, /hasAttemptedSubmit/u);
    assert.doesNotMatch(source, /isValidHostPort/u);
  });

  it("keeps stream bypass policy editing visual and controller-driven", () => {
    const page = readSource(
      "../src/views/stream-mappings/StreamBypassPolicy.vue",
    );
    const editor = readSource(
      "../src/views/stream-mappings/StreamBypassPolicyEditor.vue",
    );
    const controller = readSource(
      "../src/views/stream-mappings/useStreamBypassPolicyPage.ts",
    );
    const conditions = readSource(
      "../src/views/stream-mappings/StreamBypassConditionEditor.vue",
    );
    assert.match(page, /useStreamBypassPolicyPage/u);
    assert.match(page, /StreamBypassPolicyEditor/u);
    assert.doesNotMatch(page, /ConfigAPI|JSON\.parse|Textarea/u);
    assert.match(editor, /StreamBypassRuleGroups/u);
    assert.match(controller, /ConfigAPI|onBeforeRouteLeave/u);
    assert.match(conditions, /CidrRegionSelector/u);
    assert.doesNotMatch(conditions, /JSON\.parse|Textarea/u);
  });

  it("separates host traffic data and overlay interaction", () => {
    const source = readSource("../src/components/HostTrafficActivity.vue");
    assert.match(source, /useHostTrafficStats/u);
    assert.match(source, /useHostTrafficOverlayInteraction/u);
    assert.doesNotMatch(source, /DashboardAPI/u);
    assert.doesNotMatch(source, /setInterval/u);
  });

  it("keeps provider API workflows out of the providers table", () => {
    const source = readSource(
      "../src/views/event-center/notifications/ProvidersTab.vue",
    );
    assert.match(source, /useNotificationProviders/u);
    assert.match(source, /ProviderEditorDialog/u);
    assert.doesNotMatch(source, /EventCenterAPI/u);
    assert.doesNotMatch(source, /buildSchemaPayload/u);
  });

  it("keeps delivery, gateway, and self-signed CA workflows out of views", () => {
    const deliverySource = readSource(
      "../src/views/event-center/notifications/DeliveriesTab.vue",
    );
    const gatewaySource = readSource(
      "../src/views/system-settings/GatewaySettings.vue",
    );
    const caSource = readSource("../src/views/ssl-settings/SelfSignedCA.vue");
    const eventsSource = readSource("../src/views/event-center/EventsTab.vue");
    assert.match(deliverySource, /useNotificationDeliveries/u);
    assert.doesNotMatch(deliverySource, /EventCenterAPI/u);
    assert.match(gatewaySource, /useGatewaySettingsController/u);
    assert.doesNotMatch(gatewaySource, /ConfigAPI/u);
    assert.match(caSource, /useSelfSignedCA/u);
    assert.doesNotMatch(caSource, /ConfigAPI/u);
    assert.match(eventsSource, /useSystemEvents/u);
    assert.doesNotMatch(eventsSource, /EventCenterAPI/u);
  });

  it("keeps log resource lifecycles out of their pages", () => {
    const wafSource = readSource("../src/views/WAFLogs.vue");
    const gatewaySource = readSource("../src/views/GatewayRequestLogs.vue");
    const gatewayPaginationSource = readSource(
      "../src/views/gateway-request-logs/GatewayRequestLogsPagination.vue",
    );
    assert.match(wafSource, /useWafLogsResource/u);
    assert.match(wafSource, /WAFLogsHeader/u);
    assert.match(wafSource, /WAFLogsFilters/u);
    assert.match(wafSource, /WAFLogsPagination/u);
    assert.doesNotMatch(wafSource, /WAFAPI/u);
    assert.doesNotMatch(wafSource, /setInterval/u);
    assert.match(gatewaySource, /useGatewayRequestLogsResource/u);
    assert.match(gatewayPaginationSource, /CursorPaginationDock/u);
    assert.doesNotMatch(gatewaySource, /GatewayLogsAPI/u);
    assert.doesNotMatch(gatewaySource, /getTOTPStatus/u);
  });

  it("keeps runtime API orchestration out of the runtime presentation", () => {
    const runtimeSource = readSource(
      "../src/views/event-center/RuntimeTab.vue",
    );
    const controllerSource = readSource(
      "../src/views/event-center/useRuntimeHealth.ts",
    );
    assert.match(runtimeSource, /useRuntimeHealth/u);
    assert.doesNotMatch(runtimeSource, /RuntimeHealthAPI|EventCenterAPI/u);
    assert.match(controllerSource, /RuntimeHealthAPI/u);
    assert.match(controllerSource, /createVisibilityPoller/u);
  });

  it("keeps OIDC and ACME form workflows in focused composables", () => {
    const oidcSource = readSource("../src/views/OIDCProviderSettings.vue");
    const acmeSource = readSource(
      "../src/views/ssl-settings/AcmeApplicationDialog.vue",
    );
    assert.match(oidcSource, /useOIDCProviderManagement/u);
    assert.doesNotMatch(oidcSource, /ConfigAPI/u);
    assert.doesNotMatch(oidcSource, /normalizeScopes/u);
    assert.match(acmeSource, /useAcmeApplicationForm/u);
    assert.doesNotMatch(acmeSource, /useDnsCredentialTransfer/u);
    assert.doesNotMatch(acmeSource, /getSatisfiedCredentialScheme/u);
  });

  it("keeps ACME application and certificate actions out of the page", () => {
    const source = readSource("../src/views/ssl-settings/AcmeCert.vue");
    const workflowSource = readSource(
      "../src/views/ssl-settings/AcmeCertificateWorkflowPanels.vue",
    );
    assert.match(source, /useAcmeCertificateController/u);
    assert.match(source, /AcmeCertificateHeader/u);
    assert.match(source, /AcmeCertificateApplicationsTable/u);
    assert.match(source, /AcmeCertificateWorkflowPanels/u);
    assert.doesNotMatch(source, /<Table\b|DialogContent/u);
    assert.match(workflowSource, /AcmeApplicationDialog/u);
    assert.match(workflowSource, /AcmeJobPanel/u);
    assert.doesNotMatch(source, /AcmeAPI/u);
    assert.doesNotMatch(source, /useAsyncAction/u);
  });

  it("keeps tunnel polling and API orchestration out of tunnel views", () => {
    const frpSource = readSource("../src/views/tunnel/FrpTunnel.vue");
    const cloudflareSource = readSource(
      "../src/views/tunnel/CloudflareTunnel.vue",
    );
    assert.match(frpSource, /useFrpTunnelController/u);
    assert.doesNotMatch(frpSource, /FrpcAPI/u);
    assert.doesNotMatch(frpSource, /useTargetPolling/u);
    assert.match(cloudflareSource, /useCloudflareTunnelController/u);
    assert.doesNotMatch(cloudflareSource, /CloudflaredAPI/u);
    assert.doesNotMatch(cloudflareSource, /useTargetPolling/u);
  });

  it("keeps FRPC lifecycle and IP list resources out of presentation roots", () => {
    const frpcSource = readSource(
      "../src/views/tunnel/frp/FrpcInstancePage.vue",
    );
    const whitelistSource = readSource("../src/views/IPWhitelist.vue");
    const blacklistSource = readSource(
      "../src/views/session-management/IpBlacklistTab.vue",
    );
    const generalBlacklistSource = readSource(
      "../src/views/session-management/GeneralBlacklistTab.vue",
    );
    assert.match(frpcSource, /useFrpcInstancePage/u);
    assert.doesNotMatch(
      frpcSource,
      /FrpcAPI|ConfigAPI|createVisibilityPoller/u,
    );
    assert.match(whitelistSource, /useIpWhitelistPage/u);
    assert.match(whitelistSource, /WhitelistRecordsPanel/u);
    assert.match(whitelistSource, /WhitelistRegionGroups/u);
    assert.doesNotMatch(
      whitelistSource,
      /useLocalPagedList|formatWhitelistRemaining/u,
    );
    assert.match(blacklistSource, /useIpBlacklistPage/u);
    assert.match(blacklistSource, /IpBlacklistRecordsPanel/u);
    assert.match(blacklistSource, /IpBlacklistDetailDialog/u);
    assert.doesNotMatch(
      blacklistSource,
      /ScannerAPI|SecurityAPI|useAsyncAction/u,
    );
    assert.match(generalBlacklistSource, /useGeneralBlacklistPage/u);
    assert.match(generalBlacklistSource, /GeneralBlacklistRecordsPanel/u);
    assert.match(generalBlacklistSource, /GeneralBlacklistAddDialog/u);
    assert.doesNotMatch(
      generalBlacklistSource,
      /GeneralBlacklistAPI|usePagedSelectionList|useAsyncAction/u,
    );
  });

  it("keeps certificate and security pages presentation-focused", () => {
    const certSource = readSource("../src/views/ssl-settings/CertConfig.vue");
    const sshSource = readSource("../src/views/SSHSecurity.vue");
    assert.match(certSource, /CertificateStatusCard/u);
    assert.match(certSource, /CertificateDeploymentCard/u);
    assert.match(certSource, /ActiveCertificateDetailsCard/u);
    assert.match(sshSource, /useSSHSecurityConfig/u);
    assert.match(sshSource, /SSHSecurityConfigurationCard/u);
    assert.match(sshSource, /SSHSecurityActivityTabs/u);
    assert.match(sshSource, /SSHSecurityClearFirewallDialog/u);
    assert.doesNotMatch(sshSource, /CidrRegionSelector|DropdownMenu/u);
    assert.doesNotMatch(sshSource, /SSHSecurityAPI/u);
  });

  it("keeps maintenance workflows behind focused presentation sections", () => {
    const source = readSource(
      "../src/views/system-settings/MaintenanceSettings.vue",
    );
    const backupPanelSource = readSource(
      "../src/views/system-settings/MaintenanceBackupPanel.vue",
    );
    const dangerSource = readSource(
      "../src/views/system-settings/MaintenanceDangerZone.vue",
    );
    assert.match(source, /useMaintenanceBackupWorkflow/u);
    assert.match(source, /useMaintenanceClearData/u);
    assert.match(source, /MaintenanceBackupPanel/u);
    assert.match(source, /MaintenanceBackupDialogs/u);
    assert.match(source, /MaintenanceDangerZone/u);
    assert.doesNotMatch(source, /DialogContent|DropdownMenu/u);
    assert.match(backupPanelSource, /KNOCK_BACKUP_EXTENSION/u);
    assert.match(dangerSource, /clearAllData/u);
  });

  it("keeps IP location and session mobility models out of their pages", () => {
    const locationSource = readSource(
      "../src/views/system-settings/IpLocationSettings.vue",
    );
    const mobilitySource = readSource(
      "../src/views/session-management/mobility/SessionMobilityPage.vue",
    );
    assert.match(locationSource, /useIpLocationSettings/u);
    assert.doesNotMatch(locationSource, /IpLocationAPI/u);
    assert.doesNotMatch(locationSource, /normalizeIpLocationConfig/u);
    assert.match(mobilitySource, /useSessionMobilityPage/u);
    assert.doesNotMatch(mobilitySource, /SessionAPI/u);
    assert.doesNotMatch(mobilitySource, /buildMobilityTimeline/u);
  });

  it("keeps complex dialogs and destructive actions out of composition roots", () => {
    const reverseProxySource = readSource("../src/views/ReverseProxy.vue");
    const reverseProxyDialogsSource = readSource(
      "../src/views/reverse-proxy/ReverseProxyDialogs.vue",
    );
    const reverseProxyControllerSource = readSource(
      "../src/views/reverse-proxy/useReverseProxyPage.ts",
    );
    const whitelistSource = readSource("../src/views/IPWhitelist.vue");
    const subdomainSource = readSource("../src/views/SubdomainProxy.vue");
    const subdomainControllerSource = readSource(
      "../src/views/subdomain-proxy/useSubdomainProxyPage.ts",
    );
    const subdomainLifecycleSource = readSource(
      "../src/views/subdomain-proxy/useSubdomainProxyLifecycle.ts",
    );
    assert.match(reverseProxySource, /useReverseProxyPage/u);
    assert.match(reverseProxySource, /ReverseProxyMappingsCard/u);
    assert.match(reverseProxySource, /ReverseProxyDialogs/u);
    assert.match(reverseProxyDialogsSource, /ReverseProxyDiscoverDialog/u);
    assert.match(reverseProxyControllerSource, /useReverseProxyDiscoverFlow/u);
    assert.match(
      reverseProxyControllerSource,
      /useReverseProxyMappingActions/u,
    );
    assert.doesNotMatch(reverseProxySource, /ConfigAPI|useAsyncAction/u);
    assert.match(whitelistSource, /WhitelistAddDialog/u);
    assert.match(subdomainSource, /useSubdomainProxyPage/u);
    assert.match(subdomainSource, /SubdomainProxyOverview/u);
    assert.match(subdomainSource, /SubdomainProxyDialogs/u);
    assert.match(subdomainSource, /controller\.overview/u);
    assert.match(subdomainSource, /controller\.dialogs/u);
    assert.doesNotMatch(subdomainSource, /ConfigAPI|useAsyncAction/u);
    assert.match(subdomainControllerSource, /useSubdomainDestructiveActions/u);
    assert.match(subdomainControllerSource, /useGatewayVisibilityStatus/u);
    assert.match(subdomainControllerSource, /useSubdomainProxyLifecycle/u);
    assert.match(subdomainControllerSource, /overview:\s*\{/u);
    assert.match(subdomainControllerSource, /dialogs:\s*\{/u);
    assert.match(subdomainLifecycleSource, /if \(disposed\) return;/u);
    assert.doesNotMatch(subdomainSource, /DEFAULT_AUTH_SUBDOMAIN/u);
  });

  it("keeps subdomain visibility editing in its focused subform", () => {
    const dialogSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingDialog.vue",
    );
    const visibilitySource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingVisibilityPanel.vue",
    );
    assert.match(dialogSource, /SubdomainMappingVisibilityPanel/u);
    assert.match(dialogSource, /SubdomainMappingBasicForm/u);
    assert.doesNotMatch(dialogSource, /CidrRegionSelector/u);
    assert.doesNotMatch(dialogSource, /mapping-protocol-mode/u);
    assert.doesNotMatch(dialogSource, /visibilityCustomCidrsModel/u);
    assert.match(visibilitySource, /CidrRegionSelector/u);
    assert.match(visibilitySource, /visibilityCustomCidrsModel/u);
  });

  it("keeps notification and subdomain editor roots composition-focused", () => {
    const notificationSource = readSource(
      "../src/views/event-center/notifications/NotificationRuleEditorDialog.vue",
    );
    const mappingSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingAdvancedSettings.vue",
    );
    const advancedAuthSource = readSource(
      "../src/views/subdomain-proxy/SubdomainAdvancedAuth.vue",
    );
    const advancedAuthControllerSource = readSource(
      "../src/views/subdomain-proxy/useSubdomainAdvancedAuthPage.ts",
    );

    assert.match(notificationSource, /NotificationRuleDialogHeader/u);
    assert.match(notificationSource, /NotificationRuleEventTypes/u);
    assert.match(notificationSource, /NotificationRuleConditions/u);
    assert.match(notificationSource, /NotificationRuleTargets/u);
    assert.doesNotMatch(notificationSource, /SchemaFieldsEditor/u);
    assert.match(mappingSource, /SubdomainMappingAccessSettings/u);
    assert.match(mappingSource, /SubdomainMappingProxyProtocolSettings/u);
    assert.match(mappingSource, /SubdomainMappingVisibilityEntry/u);
    assert.doesNotMatch(
      mappingSource,
      /mapping-protocol-mode|TooltipProvider|globalVisibilityLoadError/u,
    );
    assert.match(advancedAuthSource, /useSubdomainAdvancedAuthPage/u);
    assert.match(advancedAuthSource, /SubdomainAdvancedAuthEditor/u);
    assert.doesNotMatch(advancedAuthSource, /ConfigAPI|onBeforeRouteLeave/u);
    assert.match(advancedAuthControllerSource, /ConfigAPI/u);
    assert.match(advancedAuthControllerSource, /onBeforeRouteLeave/u);
  });

  it("keeps run mode side effects in its controller", () => {
    const source = readSource(
      "../src/views/system-settings/RunModeSettings.vue",
    );
    assert.match(source, /useRunModeSettingsController/u);
    assert.doesNotMatch(source, /SystemAPI/u);
    assert.doesNotMatch(source, /ensureTunnelsStoppedForTargetMode/u);
  });

  it("keeps feature and WAF mutations out of settings views", () => {
    const featureSource = readSource(
      "../src/views/system-settings/FeaturesSettings.vue",
    );
    const wafSource = readSource(
      "../src/views/system-settings/WAFSettings.vue",
    );
    assert.match(featureSource, /useFeaturesSettings/u);
    assert.doesNotMatch(featureSource, /ConfigAPI|SystemAPI|SSHSecurityAPI/u);
    assert.doesNotMatch(featureSource, /useAsyncAction/u);
    assert.match(wafSource, /useWAFSettings/u);
    assert.match(wafSource, /WAFRuleList/u);
    assert.doesNotMatch(wafSource, /WAFAPI/u);
    assert.doesNotMatch(wafSource, /useAsyncAction/u);
  });

  it("keeps shared chart data and uPlot lifecycle outside the SFC", () => {
    const source = readSource("../src/components/charts/TimeSeriesChart.vue");
    assert.match(source, /useTimeSeriesChart/u);
    assert.match(source, /timeSeriesChartModel/u);
    assert.doesNotMatch(source, /new uPlot/u);
    assert.doesNotMatch(source, /ResizeObserver/u);
    assert.doesNotMatch(source, /MutationObserver/u);
  });

  it("keeps dashboard and authentication resources out of their pages", () => {
    const dashboardSource = readSource("../src/views/Dashboard.vue");
    const authSource = readSource("../src/views/AuthSettings.vue");
    const authControllerSource = readSource(
      "../src/views/auth-settings/useAuthSettingsPage.ts",
    );
    const loginSource = readSource(
      "../../server-auth-view/src/views/Login.vue",
    );
    assert.match(dashboardSource, /useDashboardData/u);
    assert.doesNotMatch(dashboardSource, /DashboardAPI|DDNSAPI|SecurityAPI/u);
    assert.doesNotMatch(dashboardSource, /setInterval/u);
    assert.match(authSource, /useAuthSettingsPage/u);
    assert.match(authSource, /AuthSettingsHeader/u);
    assert.match(authSource, /AuthSettingsTables/u);
    assert.match(authSource, /AuthSettingsDialogs/u);
    assert.match(authControllerSource, /useAuthSettingsResource/u);
    assert.match(authControllerSource, /useAuthModeSwitch/u);
    assert.doesNotMatch(authSource, /ConfigAPI/u);
    assert.match(loginSource, /useLoginBootstrap/u);
    assert.match(loginSource, /useCredentialLogin/u);
    assert.match(loginSource, /useOidcLogin/u);
    assert.doesNotMatch(loginSource, /AuthAPI|apiClient/u);
  });

  it("keeps update state derivation and install lifecycle out of the about page", () => {
    const source = readSource("../src/views/AboutUpdate.vue");
    const controllerSource = readSource(
      "../src/views/about-update/useAboutUpdatePage.ts",
    );
    assert.match(source, /useAboutUpdatePage/u);
    assert.match(source, /AboutUpdateDeploymentNotices/u);
    assert.match(source, /AboutUpdateVersionPanel/u);
    assert.match(source, /AboutUpdateProgressOverlay/u);
    assert.doesNotMatch(source, /useUpdateStore|useConfigStore|setTimeout/u);
    assert.match(controllerSource, /useUpdateStore/u);
    assert.match(controllerSource, /onBeforeUnmount/u);
    assert.match(controllerSource, /if \(disposed\) return/u);
  });

  it("keeps request analytics derivation and portal mutations out of pages", () => {
    const analyticsSource = readSource(
      "../src/views/request-analysis/RequestAnalyticsTab.vue",
    );
    const analyticsControllerSource = readSource(
      "../src/views/request-analysis/useRequestAnalyticsPage.ts",
    );
    const portalSource = readSource(
      "../src/views/system-settings/GatewayPortalSettings.vue",
    );
    const portalControllerSource = readSource(
      "../src/views/system-settings/gateway-portal/useGatewayPortalSettings.ts",
    );
    assert.match(analyticsSource, /useRequestAnalyticsPage/u);
    assert.match(analyticsSource, /RequestAnalyticsOverview/u);
    assert.match(analyticsSource, /RequestAnalyticsBreakdowns/u);
    assert.doesNotMatch(analyticsSource, /mapAnalyticsBuckets|GatewayLogsAPI/u);
    assert.match(analyticsControllerSource, /useGatewayRequestAnalytics/u);
    assert.match(analyticsControllerSource, /mapAnalyticsBuckets/u);
    assert.match(portalSource, /useGatewayPortalSettings/u);
    assert.match(portalSource, /GatewayPortalSettingsPanel/u);
    assert.doesNotMatch(
      portalSource,
      /ConfigAPI|useConfigStore|useAsyncAction/u,
    );
    assert.match(portalControllerSource, /ConfigAPI/u);
    assert.match(portalControllerSource, /savePortalPatch/u);
  });

  it("keeps scanner, gateway-host, and DDNS address responsibilities focused", () => {
    const scannerSource = readSource(
      "../src/views/system-settings/ScannerFirewallSettings.vue",
    );
    const scannerControllerSource = readSource(
      "../src/views/system-settings/scanner-firewall/useScannerFirewallSettings.ts",
    );
    const gatewayHostSource = readSource(
      "../src/views/system-settings/GatewayHostToggleSettings.vue",
    );
    const gatewayHostControllerSource = readSource(
      "../src/views/system-settings/gateway-host-toggle/useGatewayHostToggleSettings.ts",
    );
    const ddnsSource = readSource(
      "../src/views/ddns-management/DDNSAddressSourceFields.vue",
    );
    const ddnsInterfaceSource = readSource(
      "../src/views/ddns-management/DDNSInterfaceAddressFields.vue",
    );
    assert.match(scannerSource, /useScannerFirewallSettings/u);
    assert.match(scannerSource, /ScannerFirewallExemptions/u);
    assert.match(scannerSource, /ScannerFirewallThresholds/u);
    assert.doesNotMatch(scannerSource, /ScannerAPI|parseCidrTextarea/u);
    assert.match(scannerControllerSource, /ScannerAPI/u);
    assert.match(scannerControllerSource, /parseCidrTextarea/u);
    assert.match(gatewayHostSource, /useGatewayHostToggleSettings/u);
    assert.match(gatewayHostSource, /GatewayHostToggleTable/u);
    assert.doesNotMatch(
      gatewayHostSource,
      /useConfigStore|resolveExplicitPublicAccessEntryPort/u,
    );
    assert.match(gatewayHostControllerSource, /useConfigStore/u);
    assert.match(
      gatewayHostControllerSource,
      /resolveExplicitPublicAccessEntryPort/u,
    );
    assert.match(ddnsSource, /DDNSAddressSourceBaseFields/u);
    assert.match(ddnsSource, /DDNSStaticAddressFields/u);
    assert.match(ddnsSource, /DDNSInterfaceAddressFields/u);
    assert.doesNotMatch(ddnsSource, /ALLOW_PRIVATE_ADDRESSES_KEY/u);
    assert.match(ddnsInterfaceSource, /DDNSInterfaceSelectorEditor/u);
    assert.match(ddnsInterfaceSource, /ALLOW_PRIVATE_ADDRESSES_KEY/u);
  });

  it("keeps DDNS and terminal composition roots on focused domain modules", () => {
    const ddnsSource = readSource("../src/views/DDNSManagement.vue");
    const ddnsContentSource = readSource(
      "../src/views/ddns-management/DDNSManagementContent.vue",
    );
    const ddnsControllerSource = readSource(
      "../src/views/ddns-management/useDDNSManagementPage.ts",
    );
    const ddnsTargetDialogSource = readSource(
      "../src/views/ddns-management/DDNSTargetDialog.vue",
    );
    const terminalSource = readSource("../src/views/WebTerminal.vue");
    const terminalControllerSource = readSource(
      "../src/views/web-terminal/useWebTerminalPage.ts",
    );
    const terminalDialogsSource = readSource(
      "../src/views/web-terminal/WebTerminalDialogs.vue",
    );
    assert.match(ddnsSource, /useDDNSManagementPage/u);
    assert.match(ddnsSource, /DDNSManagementContent/u);
    assert.doesNotMatch(ddnsSource, /DDNSAPI|onBeforeRouteLeave/u);
    assert.match(ddnsControllerSource, /useDDNSResourceLoading/u);
    assert.match(ddnsControllerSource, /useDDNSPolling/u);
    assert.match(ddnsControllerSource, /useDDNSPrimaryConfigActions/u);
    assert.match(ddnsControllerSource, /useDDNSPrimaryConfigState/u);
    assert.match(ddnsControllerSource, /useDDNSCredentialTransferHint/u);
    assert.match(ddnsControllerSource, /useDDNSStatusPresentation/u);
    assert.match(ddnsControllerSource, /DDNSAPI|onBeforeRouteLeave/u);
    assert.match(ddnsContentSource, /DDNSPrimaryConfigCard/u);
    assert.match(ddnsContentSource, /DDNSExtraTargetsCard/u);
    assert.match(ddnsTargetDialogSource, /DDNSTargetBasicFields/u);
    assert.match(ddnsTargetDialogSource, /DDNSTargetAddressFields/u);
    assert.match(ddnsTargetDialogSource, /DDNSTargetProviderFields/u);
    assert.doesNotMatch(ddnsTargetDialogSource, /ALLOW_PRIVATE_ADDRESSES_KEY/u);
    assert.match(terminalSource, /useWebTerminalPage/u);
    assert.match(terminalSource, /WebTerminalWorkspace/u);
    assert.match(terminalSource, /WebTerminalDialogs/u);
    assert.doesNotMatch(terminalSource, /TerminalAPI|onMounted/u);
    assert.match(terminalControllerSource, /useTerminalSessionController/u);
    assert.match(terminalControllerSource, /useTerminalInputQueue/u);
    assert.match(terminalControllerSource, /useTerminalResizeQueue/u);
    assert.match(terminalControllerSource, /useTerminalEmulator/u);
    assert.doesNotMatch(terminalControllerSource, /ensureGhostty/u);
    assert.doesNotMatch(
      terminalControllerSource,
      /createTerminalFitController/u,
    );
    assert.match(terminalDialogsSource, /TerminalRenameDialog/u);
    assert.match(terminalDialogsSource, /TerminalSendDialog/u);
  });

  it("keeps field groups, cleanup results, and status indicators behind composition roots", () => {
    const targetAddressSource = readSource(
      "../src/views/ddns-management/DDNSTargetAddressFields.vue",
    );
    const staleDialogSource = readSource(
      "../src/components/StaleHostMappingsCleanupDialog.vue",
    );
    const staleControllerSource = readSource(
      "../src/components/stale-host-mappings/useStaleHostMappingsCleanupDialog.ts",
    );
    const statusSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingStatusIndicators.vue",
    );
    const statusTooltipSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingStatusTooltip.vue",
    );

    assert.match(targetAddressSource, /DDNSTargetAddressBaseFields/u);
    assert.match(targetAddressSource, /DDNSTargetStaticAddressFields/u);
    assert.match(targetAddressSource, /DDNSTargetInterfaceAddressFields/u);
    assert.doesNotMatch(
      targetAddressSource,
      /ALLOW_PRIVATE_ADDRESSES_KEY|DDNSInterfaceSelectorEditor/u,
    );
    assert.match(staleDialogSource, /useStaleHostMappingsCleanupDialog/u);
    assert.match(staleDialogSource, /StaleHostMappingsResults/u);
    assert.doesNotMatch(
      staleDialogSource,
      /<Table|useStaleHostMappingsCleanup\(/u,
    );
    assert.match(staleControllerSource, /useStaleHostMappingsCleanup\(/u);
    assert.match(statusSource, /SubdomainMappingAvailabilityIndicators/u);
    assert.match(statusSource, /SubdomainMappingAccessIndicators/u);
    assert.match(statusSource, /SubdomainMappingSecurityIndicators/u);
    assert.doesNotMatch(statusSource, /TooltipProvider|BrickWall|ShieldCheck/u);
    assert.match(statusTooltipSource, /TooltipProvider/u);
    assert.match(statusTooltipSource, /handleMappingStatusTooltipOpenChange/u);
  });

  it("keeps mapping tables, optimization panels, and auth conditions focused", () => {
    const cardSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingsCard.vue",
    );
    const tableSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingsTable.vue",
    );
    const optimizationSource = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
    const authGroupsSource = readSource(
      "../src/views/subdomain-proxy/AdvancedAuthRuleGroups.vue",
    );

    assert.match(cardSource, /SubdomainMappingsTable/u);
    assert.doesNotMatch(cardSource, /SubdomainMappingGroupRows|TableRow/u);
    assert.match(tableSource, /SubdomainMappingGroupHeaderRow/u);
    assert.match(tableSource, /SubdomainMappingTableRow/u);
    assert.doesNotMatch(tableSource, /HostTrafficActivity|GripVertical/u);
    assert.match(optimizationSource, /CloudflareOptimizationOverview/u);
    assert.match(optimizationSource, /CloudflareOptimizationDomains/u);
    assert.match(optimizationSource, /CloudflareOptimizationTechnicalStatus/u);
    assert.doesNotMatch(
      optimizationSource,
      /useConfirmationDialog|type CloudflareOptimizationDomain\b/u,
    );
    assert.match(authGroupsSource, /AdvancedAuthRuleGroupCard/u);
    assert.doesNotMatch(
      authGroupsSource,
      /CidrRegionSelector|AdvancedAuthHeaderNameField/u,
    );
  });

  it("guards deferred work and polling startup after unmount", () => {
    const dashboardSource = readSource(
      "../src/views/dashboard/useDashboardData.ts",
    );
    const terminalPageSource = readSource(
      "../src/views/web-terminal/useWebTerminalPage.ts",
    );
    const terminalSource = readSource(
      "../src/views/web-terminal/useTerminalEmulator.ts",
    );
    const loginSource = readSource(
      "../../server-auth-view/src/composables/useLoginBootstrap.ts",
    );
    const ddnsSource = readSource(
      "../src/views/ddns-management/useDDNSManagementPage.ts",
    );
    const frpSource = readSource(
      "../src/views/tunnel/frp/useFrpTunnelController.ts",
    );
    const cloudflareSource = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    const deepMonitorSource = readSource("../src/views/DeepMonitor.vue");
    const wafLogsSource = readSource(
      "../src/views/waf-logs/useWafLogsResource.ts",
    );
    const frpcInstanceSource = readSource(
      "../src/views/tunnel/frp/useFrpcInstancePage.ts",
    );
    const acmeJobSource = readSource(
      "../src/views/ssl-settings/useAcmeJobPolling.ts",
    );
    const layoutSource = readSource("../src/views/Layout.vue");
    const systemClockStoreSource = readSource("../src/store/systemClock.ts");
    const updateStoreSource = readSource("../src/store/update.ts");
    assert.match(dashboardSource, /clearTimeout\(ddnsLoadTimer\)/u);
    assert.match(terminalPageSource, /if \(disposed\) return/u);
    assert.match(terminalSource, /initializationPromise/u);
    assert.match(terminalSource, /disposed \|\| !mountElement/u);
    assert.match(loginSource, /onBeforeUnmount/u);
    assert.match(loginSource, /if \(disposed\) return/u);
    assert.match(ddnsSource, /initialized && !isDisposed/u);
    assert.match(frpSource, /if \(isDisposed\) return;/u);
    assert.match(cloudflareSource, /if \(isDisposed\) return;/u);
    assert.match(deepMonitorSource, /if \(isDisposed\) return;/u);
    assert.match(wafLogsSource, /if \(isDisposed\) return;/u);
    assert.match(frpcInstanceSource, /if \(!isDisposed\) startPolling\(\)/u);
    assert.match(acmeJobSource, /if \(isDisposed\) return;/u);
    assert.match(layoutSource, /if \(isDisposed\) return;/u);
    assert.match(systemClockStoreSource, /createPollingLifecycle/u);
    assert.match(updateStoreSource, /createPollingLifecycle/u);
  });

  it("separates OIDC binding polling from passkey management", () => {
    const source = readSource("../src/views/PasskeySettings.vue");
    assert.match(source, /useOidcBindingWorkflow/u);
    assert.match(source, /OidcInviteDialog/u);
    assert.doesNotMatch(source, /createOIDCInvite/u);
    assert.doesNotMatch(source, /getOIDCBindings/u);
  });

  it("keeps settings-only view models outside their pages", () => {
    const sessionSource = readSource(
      "../src/views/system-settings/SessionSettings.vue",
    );
    const smartConnectSource = readSource(
      "../src/views/system-settings/SmartConnectSettings.vue",
    );
    const captchaSource = readSource(
      "../src/views/system-settings/CaptchaSettings.vue",
    );
    const fnosSource = readSource(
      "../src/views/system-settings/FnosSettings.vue",
    );
    const fnosControllerSource = readSource(
      "../src/views/system-settings/fnos-settings/useFnosSettingsController.ts",
    );
    const sessionControllerSource = readSource(
      "../src/views/system-settings/session-settings/useSessionSettingsController.ts",
    );
    assert.match(sessionSource, /useSessionSettingsController/u);
    assert.match(sessionSource, /SessionAccessPolicyPanel/u);
    assert.doesNotMatch(sessionSource, /ConfigAPI|useAsyncAction/u);
    assert.match(sessionControllerSource, /useSessionCookieScope/u);
    assert.match(sessionControllerSource, /sessionDurationModel/u);
    assert.doesNotMatch(sessionSource, /normalizeDomainName/u);
    assert.match(smartConnectSource, /useSmartConnectViewModel/u);
    assert.match(smartConnectSource, /smartConnectModel/u);
    assert.match(smartConnectSource, /SmartConnectFormPanel/u);
    assert.doesNotMatch(smartConnectSource, /dnsmasqNeedsInitialization/u);
    assert.match(captchaSource, /PowCaptchaSettingsFields/u);
    assert.match(captchaSource, /TurnstileCaptchaSettingsFields/u);
    assert.doesNotMatch(captchaSource, /baseDifficultySelection/u);
    assert.match(fnosSource, /useFnosSettingsController/u);
    assert.doesNotMatch(fnosSource, /SystemAPI|useAsyncAction/u);
    assert.match(fnosControllerSource, /useFnosNetworkTuningViewModel/u);
    assert.doesNotMatch(fnosControllerSource, /displaySysctlValue/u);
  });
});
