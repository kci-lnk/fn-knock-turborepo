import { join } from "node:path";

import { dataPath } from "../AppDirManager";
import type { AutoHttpsConfig } from "../auto-https-redirect";
import type { LocaleConfig } from "../../../../../packages/i18n/src";
import {
  DEFAULT_APPEARANCE_CONFIG,
  type AppearanceConfig,
} from "../../../../../packages/admin-shared/src/utils/appearance";
import type {
  AuthCredentialSettings,
  DashboardDisplayConfig,
  EventSystemConfig,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayHostResponseConfig,
  GatewayHostResponseRuntimeState,
  GatewayLoggingSettings,
  GatewayPortalConfig,
  GatewayProxyHeadersConfig,
  GatewayProxyHeadersRuntimeState,
  GatewayVisibilityConfig,
  GatewayVisibilityRuntimeState,
  IpLocationApiConfig,
  ReverseProxyThrottleConfig,
  ReverseProxyTrustedIPRuntimeState,
  ScanDiscoveryConfig,
  SmartConnectConfig,
  SmartConnectRuntimeState,
  WAFConfig,
  ProtocolMappingFeatureConfig,
  RunModePromptPreferences,
} from "./types";

export const DEFAULT_IP_LOCATION_API_CONFIG: IpLocationApiConfig = {
  ip_lookup_mode: "online",
  ip_lookup_url: "https://ipaddress.fnknock.cn/api/v1",
  cidr_mode: "online",
  cidr_url: "https://cidr.wxlnk.com/api/v1",
};

export const DEFAULT_AUTH_CREDENTIAL_SETTINGS: AuthCredentialSettings = {
  session_ttl_seconds: 24 * 3600,
  remember_me_ttl_seconds: 365 * 24 * 3600,
  post_login_ip_grant_mode: "follow_session",
  post_login_ip_grant_ttl_seconds: 3600,
  session_ip_mobility_enabled: false,
  session_ip_mobility_window_seconds: 20 * 60,
  passkey_bind_prompt_enabled: true,
};

export const DEFAULT_GATEWAY_LOGGING_SETTINGS: GatewayLoggingSettings = {
  enabled: false,
  max_days: 7,
};

export const DEFAULT_GATEWAY_CONFIG_DIR =
  process.env.FN_KNOCK_GATEWAY_CONFIG_DIR?.trim() ||
  process.env.GATEWAY_CONFIG_DIR?.trim() ||
  dataPath;

export const DEFAULT_WAF_CONFIG: WAFConfig = {
  enabled: false,
  system_rules_auto_update_enabled: true,
  common_location_exempt_enabled: false,
  mode: "blocking",
  active_bundle_id: "local",
  rules_dir: join(DEFAULT_GATEWAY_CONFIG_DIR, "waf"),
  paranoia_level: 1,
  executing_paranoia_level: 1,
  inbound_anomaly_threshold: 5,
  outbound_anomaly_threshold: 4,
  request_body_access: true,
  request_body_limit_bytes: 131072,
  request_body_in_memory_limit_bytes: 65536,
  response_body_access: false,
  disabled_hosts: [],
  disabled_path_prefixes: [],
  log_retention_days: 7,
  drain_interval_seconds: 2,
  updated_at: null,
};

export const DEFAULT_GATEWAY_VISIBILITY_CONFIG: GatewayVisibilityConfig = {
  enabled: false,
  selections: [],
  custom_cidrs: [],
};

export const DEFAULT_GATEWAY_VISIBILITY_RUNTIME_STATE: GatewayVisibilityRuntimeState =
  {
    enabled: false,
    cidrs: [],
    updated_at: null,
  };

export const DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG: GatewayProxyHeadersConfig = {
  disabled_hosts: [],
};

export const DEFAULT_GATEWAY_PROXY_HEADERS_RUNTIME_STATE: GatewayProxyHeadersRuntimeState =
  {
    enabled: false,
    omit_targets: [],
    updated_at: null,
  };

export const DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG: GatewayHostResponseConfig = {
  disabled_hosts: [],
};

export const DEFAULT_GATEWAY_PORTAL_CONFIG: GatewayPortalConfig = {
  enabled: true,
  display_style: "domain",
  show_app_icon: false,
};

export const DEFAULT_DASHBOARD_DISPLAY_CONFIG: DashboardDisplayConfig = {
  show_entry_status_module: true,
};

export const DEFAULT_APPEARANCE_CONFIG_FOR_ADMIN: AppearanceConfig = {
  ...DEFAULT_APPEARANCE_CONFIG,
};

export const DEFAULT_AUTO_HTTPS_CONFIG: AutoHttpsConfig = {
  enabled: false,
};

export const DEFAULT_GATEWAY_HOST_RESPONSE_RUNTIME_STATE: GatewayHostResponseRuntimeState =
  {
    enabled: false,
    omit_targets: [],
    updated_at: null,
  };

export const DEFAULT_REVERSE_PROXY_TRUSTED_IP_RUNTIME_STATE: ReverseProxyTrustedIPRuntimeState =
  {
    enabled: false,
    items: [],
    cidrs: [],
    updated_at: null,
  };

export const DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG: ReverseProxyThrottleConfig =
  {
    enabled: true,
    requests_per_second: 100,
    burst: 200,
    block_seconds: 30,
  };

export const DEFAULT_EVENT_SYSTEM_CONFIG: EventSystemConfig = {
  enabled: true,
  retention_days: 30,
  rules: {
    login_failure: {
      enabled: true,
    },
    ip_drift: {
      enabled: true,
    },
    scanner_blocked: {
      enabled: true,
    },
    ddns_update: {
      enabled: true,
    },
    gateway_throttle_block: {
      enabled: true,
    },
    waf_blocked: {
      enabled: true,
    },
    app_update_available: {
      enabled: true,
    },
    frp_tunnel: {
      enabled: true,
    },
    cloudflared_tunnel: {
      enabled: true,
    },
    ssh_login_success: {
      enabled: true,
    },
    ssh_login_failure: {
      enabled: true,
    },
    ssh_ip_blocked: {
      enabled: true,
    },
    cpu_alert: {
      enabled: true,
      threshold_percent: 80,
      recover_percent: 60,
      sample_interval_seconds: 5,
      sustain_seconds: 30,
    },
    memory_alert: {
      enabled: true,
      threshold_percent: 80,
      recover_percent: 60,
      sample_interval_seconds: 5,
      sustain_seconds: 30,
    },
  },
};

export const DEFAULT_LOCALE_CONFIG: LocaleConfig = {
  default_locale: "zh-CN",
};

export const DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG: ProtocolMappingFeatureConfig =
  {
    enabled: false,
  };

export const DEFAULT_SMART_CONNECT_CONFIG: SmartConnectConfig = {
  enabled: false,
  selected_ipv4: "",
};

export const DEFAULT_SCAN_DISCOVERY_CONFIG: ScanDiscoveryConfig = {
  custom_cidrs: [],
  selected_cidrs: [],
};

export const DEFAULT_SMART_CONNECT_RUNTIME_STATE: SmartConnectRuntimeState = {
  selected_ipv4: "",
  synced_domains: [],
  managed_rule_count: 0,
  last_sync_at: null,
  last_sync_error: null,
};

export const DEFAULT_RUN_MODE_PROMPT_PREFERENCES: RunModePromptPreferences = {
  directToReverseProxy: false,
  reverseProxyToDirect: false,
  switchToSubdomain: false,
  subdomainToReverseProxy: false,
};

export const DEFAULT_FNOS_SHARE_BYPASS_CONFIG: FnosShareBypassConfig = {
  enabled: false,
  upstream_timeout_ms: 2500,
  validation_cache_ttl_seconds: 30,
  validation_lock_ttl_seconds: 5,
  session_ttl_seconds: 300,
};

export const DEFAULT_FNOS_PORT_ICON_HIJACK_CONFIG: FnosPortIconHijackConfig = {
  enabled: false,
  updated_at: null,
};
