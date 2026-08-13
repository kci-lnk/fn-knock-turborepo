import type { components as ApiContractComponents } from "@fn-knock/api-contract";

export type ProxyProtocolForce =
  ApiContractComponents["schemas"]["ProxyProtocolForceData"];

export type ReverseProxyThrottleConfig =
  ApiContractComponents["schemas"]["GatewayReverseProxyThrottleData"];

type GatewayVisibilitySelectionContract =
  ApiContractComponents["schemas"]["GatewayVisibilitySelectionData"];

export type GatewayVisibilitySelection = Omit<
  GatewayVisibilitySelectionContract,
  "operator"
> & {
  operator?: import("./cidr").CidrOperator | null;
};

export type GatewayVisibilitySummary =
  ApiContractComponents["schemas"]["GatewayVisibilitySummaryData"];

export type GatewayVisibilityConfig = Omit<
  ApiContractComponents["schemas"]["GatewayVisibilityConfigData"],
  "selections"
> & {
  selections: GatewayVisibilitySelection[];
};

export type GatewayVisibilityDetails = Omit<
  ApiContractComponents["schemas"]["GatewayVisibilityDetailsData"],
  "config" | "summary"
> & {
  config: GatewayVisibilityConfig;
  summary: GatewayVisibilitySummary;
};

export type SSHSecurityBlockDurationUnit =
  ApiContractComponents["schemas"]["SshSecurityConfigData"]["block_duration_unit"];
export type SSHSecuritySelection =
  ApiContractComponents["schemas"]["SshSecurityConfigData"]["allowed_regions"][number];
export type SSHSecurityConfig =
  ApiContractComponents["schemas"]["SshSecurityConfigData"];
export type SSHSecuritySummary =
  ApiContractComponents["schemas"]["SshSecuritySummaryData"];
export type SSHSecurityDetails =
  ApiContractComponents["schemas"]["SshSecurityDetailsData"];
export type SSHLoginLogEntry =
  ApiContractComponents["schemas"]["SshLoginLogEntryData"];
export type SSHLoginLogListPayload =
  ApiContractComponents["schemas"]["SshLoginLogListData"];
export type SSHSecurityBlockReason =
  ApiContractComponents["schemas"]["SshSecurityBlockData"]["reason"];
export type SSHSecurityBlockRecord =
  ApiContractComponents["schemas"]["SshSecurityBlockData"];
export type SSHSecurityBlockListPayload =
  ApiContractComponents["schemas"]["SshSecurityBlockListData"];
export type SSHSecurityFirewallSyncResult =
  ApiContractComponents["schemas"]["SshFirewallSyncData"];
export type SSHSecurityFirewallClearResult =
  ApiContractComponents["schemas"]["SshFirewallClearData"];

export type GatewayProxyHeadersConfig =
  ApiContractComponents["schemas"]["GatewayProxyHeadersConfigData"];
export type GatewayProxyHeadersItem =
  ApiContractComponents["schemas"]["GatewayProxyHeadersItemData"];
export type GatewayProxyHeadersAvailability =
  ApiContractComponents["schemas"]["GatewayProxyHeadersAvailabilityData"];
export type GatewayProxyHeadersSummary =
  ApiContractComponents["schemas"]["GatewayProxyHeadersSummaryData"];
export type GatewayProxyHeadersDetails =
  ApiContractComponents["schemas"]["GatewayProxyHeadersDetailsData"];

export type GatewayHostResponseConfig =
  ApiContractComponents["schemas"]["GatewayHostResponseConfigData"];
export type GatewayHostResponseItem =
  ApiContractComponents["schemas"]["GatewayHostResponseItemData"];
export type GatewayHostResponseAvailability =
  ApiContractComponents["schemas"]["GatewayHostResponseAvailabilityData"];
export type GatewayHostResponseSummary =
  ApiContractComponents["schemas"]["GatewayHostResponseSummaryData"];
export type GatewayHostResponseDetails =
  ApiContractComponents["schemas"]["GatewayHostResponseDetailsData"];

export type GatewayPortalConfig =
  ApiContractComponents["schemas"]["GatewayPortalData"];
export type GatewayPortalDisplayStyle = GatewayPortalConfig["display_style"];
export type GatewayPortalIconDragMode = GatewayPortalConfig["icon_drag_mode"];
export type GatewayPortalVersion = GatewayPortalConfig["version"];

export type GatewayUnmatchedRouteConfig =
  ApiContractComponents["schemas"]["GatewayUnmatchedRouteData"];
export type GatewayUnmatchedRouteBehavior =
  GatewayUnmatchedRouteConfig["behavior"];
export type GatewayUpstreamErrorDetail =
  GatewayUnmatchedRouteConfig["upstream_error_detail"];

export type GatewayCrawlerBlockerConfig =
  ApiContractComponents["schemas"]["GatewayCrawlerBlockerData"];

export type GatewaySettings =
  ApiContractComponents["schemas"]["GatewaySettingsData"];

export type ThreatOverview =
  ApiContractComponents["schemas"]["SecurityOverviewData"];
