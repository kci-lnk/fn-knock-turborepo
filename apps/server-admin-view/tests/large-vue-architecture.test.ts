import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const lineCount = (source: string) => source.trimEnd().split(/\r?\n/u).length;

const pageBudgets = [
  ["../src/components/ScanDiscoveryIntensityDialog.vue", 620],
  ["../src/views/event-center/notifications/RulesTab.vue", 540],
  ["../src/views/system-settings/RunModeSettings.vue", 460],
  ["../src/views/ssl-settings/AcmeCert.vue", 530],
  ["../src/views/WAFLogs.vue", 610],
  ["../src/views/GatewayRequestLogs.vue", 740],
  ["../src/views/Dashboard.vue", 550],
  ["../src/views/DDNSManagement.vue", 700],
  ["../src/views/AuthSettings.vue", 580],
  ["../../server-auth-view/src/views/Login.vue", 590],
  ["../src/views/Layout.vue", 460],
  ["../src/views/WebTerminal.vue", 530],
  ["../src/views/system-settings/WAFSettings.vue", 410],
  ["../src/views/system-settings/FeaturesSettings.vue", 180],
  ["../src/components/charts/TimeSeriesChart.vue", 130],
  ["../src/views/system-settings/MaintenanceSettings.vue", 490],
  ["../src/views/OIDCProviderSettings.vue", 360],
  ["../src/views/ssl-settings/AcmeApplicationDialog.vue", 360],
  ["../src/views/tunnel/FrpTunnel.vue", 450],
  ["../src/views/tunnel/CloudflareTunnel.vue", 400],
  ["../src/views/ssl-settings/CertConfig.vue", 470],
  ["../src/views/SSHSecurity.vue", 480],
  ["../src/views/system-settings/IpLocationSettings.vue", 420],
  ["../src/views/session-management/mobility/SessionMobilityPage.vue", 390],
  ["../src/views/ReverseProxy.vue", 560],
  ["../src/views/IPWhitelist.vue", 570],
  ["../src/views/SubdomainProxy.vue", 760],
  ["../src/views/event-center/notifications/ProvidersTab.vue", 230],
  ["../src/views/system-settings/GatewayLocationsSettings.vue", 560],
  ["../src/views/StreamMappings.vue", 390],
  ["../src/components/HostTrafficActivity.vue", 420],
  ["../src/views/system-settings/SessionSettings.vue", 550],
  ["../src/views/system-settings/SmartConnectSettings.vue", 590],
  ["../src/views/PasskeySettings.vue", 360],
  ["../src/views/system-settings/FnosSettings.vue", 540],
  ["../src/views/subdomain-proxy/SubdomainMappingDialog.vue", 690],
  ["../src/views/ssl-settings/SelfSignedCA.vue", 380],
  ["../src/views/system-settings/GatewaySettings.vue", 280],
  ["../src/views/event-center/notifications/DeliveriesTab.vue", 330],
  ["../src/views/event-center/EventsTab.vue", 430],
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
    assert.match(source, /useGatewayLocationEditor/u);
    assert.match(source, /GatewayLocationHostPickerDialog/u);
    assert.doesNotMatch(source, /forbiddenResponseHeaders/u);
    assert.doesNotMatch(source, /cleanHostLocationPath/u);
  });

  it("keeps stream mapping form state inside its editor component", () => {
    const source = readSource("../src/views/StreamMappings.vue");
    const tableSource = readSource(
      "../src/views/stream-mappings/StreamMappingTable.vue",
    );
    assert.match(source, /StreamMappingEditorDialog/u);
    assert.match(source, /StreamMappingDisabledAlert/u);
    assert.match(source, /StreamMappingTable/u);
    assert.match(tableSource, /InlineCommentEditor/u);
    assert.match(source, /streamMappingModel/u);
    assert.doesNotMatch(source, /hasAttemptedSubmit/u);
    assert.doesNotMatch(source, /isValidHostPort/u);
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
    assert.match(wafSource, /useWafLogsResource/u);
    assert.doesNotMatch(wafSource, /WAFAPI/u);
    assert.doesNotMatch(wafSource, /setInterval/u);
    assert.match(gatewaySource, /useGatewayRequestLogsResource/u);
    assert.doesNotMatch(gatewaySource, /GatewayLogsAPI/u);
    assert.doesNotMatch(gatewaySource, /getTOTPStatus/u);
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
    assert.match(source, /useAcmeCertificateController/u);
    assert.match(source, /AcmeApplicationDialog/u);
    assert.match(source, /AcmeJobPanel/u);
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

  it("keeps certificate and security pages presentation-focused", () => {
    const certSource = readSource("../src/views/ssl-settings/CertConfig.vue");
    const sshSource = readSource("../src/views/SSHSecurity.vue");
    assert.match(certSource, /CertificateStatusCard/u);
    assert.match(certSource, /CertificateDeploymentCard/u);
    assert.match(certSource, /ActiveCertificateDetailsCard/u);
    assert.match(sshSource, /useSSHSecurityConfig/u);
    assert.doesNotMatch(sshSource, /SSHSecurityAPI/u);
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
    const whitelistSource = readSource("../src/views/IPWhitelist.vue");
    const subdomainSource = readSource("../src/views/SubdomainProxy.vue");
    assert.match(reverseProxySource, /ReverseProxyDiscoverDialog/u);
    assert.match(whitelistSource, /WhitelistAddDialog/u);
    assert.match(subdomainSource, /useSubdomainDestructiveActions/u);
    assert.match(subdomainSource, /useGatewayVisibilityStatus/u);
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
    assert.doesNotMatch(dialogSource, /CidrRegionSelector/u);
    assert.doesNotMatch(dialogSource, /visibilityCustomCidrsModel/u);
    assert.match(visibilitySource, /CidrRegionSelector/u);
    assert.match(visibilitySource, /visibilityCustomCidrsModel/u);
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
    const loginSource = readSource(
      "../../server-auth-view/src/views/Login.vue",
    );
    assert.match(dashboardSource, /useDashboardData/u);
    assert.doesNotMatch(dashboardSource, /DashboardAPI|DDNSAPI|SecurityAPI/u);
    assert.doesNotMatch(dashboardSource, /setInterval/u);
    assert.match(authSource, /useAuthSettingsResource/u);
    assert.match(authSource, /useAuthModeSwitch/u);
    assert.doesNotMatch(authSource, /ConfigAPI/u);
    assert.match(loginSource, /useLoginBootstrap/u);
    assert.match(loginSource, /useCredentialLogin/u);
    assert.match(loginSource, /useOidcLogin/u);
    assert.doesNotMatch(loginSource, /AuthAPI|apiClient/u);
  });

  it("keeps DDNS and terminal composition roots on focused domain modules", () => {
    const ddnsSource = readSource("../src/views/DDNSManagement.vue");
    const terminalSource = readSource("../src/views/WebTerminal.vue");
    assert.match(ddnsSource, /useDDNSResourceLoading/u);
    assert.match(ddnsSource, /useDDNSPolling/u);
    assert.match(ddnsSource, /useDDNSPrimaryConfigActions/u);
    assert.match(ddnsSource, /useDDNSPrimaryConfigState/u);
    assert.match(ddnsSource, /useDDNSCredentialTransferHint/u);
    assert.match(ddnsSource, /useDDNSStatusPresentation/u);
    assert.match(ddnsSource, /DDNSPrimaryConfigCard/u);
    assert.match(ddnsSource, /DDNSExtraTargetsCard/u);
    assert.match(terminalSource, /useTerminalSessionController/u);
    assert.match(terminalSource, /useTerminalInputQueue/u);
    assert.match(terminalSource, /useTerminalResizeQueue/u);
    assert.match(terminalSource, /useTerminalEmulator/u);
    assert.doesNotMatch(terminalSource, /ensureGhostty/u);
    assert.doesNotMatch(terminalSource, /createTerminalFitController/u);
    assert.match(terminalSource, /TerminalRenameDialog/u);
    assert.match(terminalSource, /TerminalSendDialog/u);
  });

  it("guards deferred dashboard, terminal, and login work after unmount", () => {
    const dashboardSource = readSource(
      "../src/views/dashboard/useDashboardData.ts",
    );
    const terminalPageSource = readSource("../src/views/WebTerminal.vue");
    const terminalSource = readSource(
      "../src/views/web-terminal/useTerminalEmulator.ts",
    );
    const loginSource = readSource(
      "../../server-auth-view/src/composables/useLoginBootstrap.ts",
    );
    assert.match(dashboardSource, /clearTimeout\(ddnsLoadTimer\)/u);
    assert.match(terminalPageSource, /if \(disposed\) return/u);
    assert.match(terminalSource, /initializationPromise/u);
    assert.match(terminalSource, /disposed \|\| !mountElement/u);
    assert.match(loginSource, /onBeforeUnmount/u);
    assert.match(loginSource, /if \(disposed\) return/u);
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
    const fnosSource = readSource(
      "../src/views/system-settings/FnosSettings.vue",
    );
    assert.match(sessionSource, /useSessionCookieScope/u);
    assert.match(sessionSource, /sessionDurationModel/u);
    assert.doesNotMatch(sessionSource, /normalizeDomainName/u);
    assert.match(smartConnectSource, /useSmartConnectViewModel/u);
    assert.match(smartConnectSource, /smartConnectModel/u);
    assert.doesNotMatch(smartConnectSource, /dnsmasqNeedsInitialization/u);
    assert.match(fnosSource, /useFnosNetworkTuningViewModel/u);
    assert.doesNotMatch(fnosSource, /displaySysctlValue/u);
  });
});
