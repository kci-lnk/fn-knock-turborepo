import type { WAFConfig } from "./types/waf";
import type { AppearanceConfig } from "@frontend-core/appearance";

export type { AppearanceConfig } from "@frontend-core/appearance";

export interface ProxyMapping {
  path: string;
  target: string;
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
}

export type RunType = 0 | 1 | 3;
export type ReverseProxySubmode = "path" | "subdomain";
export type LocaleCode = "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP";

export interface LocaleConfig {
  default_locale: LocaleCode;
}

export interface WelcomeGuideStatus {
  completed: boolean;
  completed_at: string | null;
}

export type DeploymentTarget =
  | "fpk"
  | "docker"
  | "openwrt"
  | "linux"
  | "synology"
  | "windows"
  | "dev";

export interface RuntimeProfile {
  deployment_target: DeploymentTarget;
  is_docker: boolean;
  is_linux: boolean;
  is_windows: boolean;
  is_root_process: boolean;
}

export interface RuntimeCapabilities {
  direct_mode_available: boolean;
  host_firewall_available: boolean;
  smart_connect_available: boolean;
  fnos_certificate_sync_available?: boolean;
  system_clock_sync_available: boolean;
  self_update_available: boolean;
  terminal_available: boolean;
  shared_root_available: boolean;
  acme_available?: boolean;
  acme_resource_required?: boolean;
  cloudflared_available?: boolean;
  frpc_available?: boolean;
  ssh_security_available?: boolean;
  system_resource_monitor_available?: boolean;
  desktop_update_managed?: boolean;
}

export interface DockerAdminBootstrapState {
  deployment_target: DeploymentTarget;
  enabled: boolean;
  password_configured: boolean;
  authenticated: boolean;
  auth_source: "panel_session" | "reauth_session" | null;
  session_expires_at: string | null;
  locale: LocaleConfig;
  appearance: AppearanceConfig;
}

export type HostAccessMode = "login_first" | "strict_whitelist";
export type HostProtocolMode = "auto" | "http1" | "http2";
export type HostServiceRole = "app" | "auth";
export type StreamMappingProtocol = "tcp" | "udp";

export interface HostMappingBasicAuth {
  enabled: boolean;
  username: string;
  password: string;
}

export interface HostMappingAvailability {
  enabled: boolean;
  start_time: string;
  end_time: string;
}

export type HostVisibilityMode = "inherit" | "custom" | "disabled";

export interface HostMappingVisibility {
  mode: HostVisibilityMode;
  selections: GatewayVisibilitySelection[];
  custom_cidrs: string[];
  cidrs: string[];
}

export type AdvancedAuthConditionTarget =
  | "source_ip"
  | "source_region"
  | "url_path"
  | "request_header"
  | "query_parameter"
  | "http_method";

export type AdvancedAuthOperator =
  | "equals"
  | "not_equals"
  | "in_cidr"
  | "not_in_cidr"
  | "in"
  | "not_in"
  | "exists"
  | "not_exists"
  | "prefix"
  | "not_prefix"
  | "contains"
  | "not_contains"
  | "starts_with"
  | "not_starts_with"
  | "ends_with"
  | "not_ends_with"
  | "regex"
  | "not_regex";

export interface AdvancedAuthCondition {
  id: string;
  target: AdvancedAuthConditionTarget;
  operator: AdvancedAuthOperator;
  name?: string;
  values?: string[];
  selections: GatewayVisibilitySelection[];
  /** Resolved CIDRs are returned by the control plane and are read-only. */
  cidrs?: string[];
  resolved_at?: string;
  cidr_source?: string;
  cidr_source_fingerprint?: string;
}

export interface AdvancedAuthRuleGroup {
  id: string;
  conditions: AdvancedAuthCondition[];
}

export interface AdvancedAuthConfig {
  enabled: boolean;
  idle_ttl_seconds: number;
  max_lifetime_seconds: number;
  policy_version?: string;
  groups: AdvancedAuthRuleGroup[];
  compiled_at?: string;
  cidr_source?: string;
  cidr_source_fingerprint?: string;
}

export type HostLocationMatch = "exact" | "prefix";
export type HostLocationAction = "proxy" | "response";

export interface HostLocationResponse {
  status: number;
  content_type: string;
  headers: Record<string, string>;
  body: string;
}

export interface HostLocation {
  path: string;
  match: HostLocationMatch;
  action: HostLocationAction;
  target: string;
  strip_path: boolean;
  rewrite_html: boolean;
  response: HostLocationResponse;
}

export interface HostMapping {
  host: string;
  target: string;
  waf_enabled: boolean;
  use_auth: boolean;
  access_mode: HostAccessMode;
  suppress_toolbar: boolean;
  preserve_host: boolean;
  is_default: boolean;
  disabled: boolean;
  availability: HostMappingAvailability | null;
  visibility: HostMappingVisibility;
  protocol_mode: HostProtocolMode;
  basic_auth: HostMappingBasicAuth;
  locations: HostLocation[];
  service_role: HostServiceRole;
  title: string;
  title_override: string;
  favicon: string;
  advanced_auth?: AdvancedAuthConfig;
}

export interface HostMappingRefreshSummary {
  updated: number;
  failed: number;
  skipped: number;
}

export interface UrlMetadataPreview {
  title: string;
  favicon: string;
  finalUrl: string;
}

export interface StreamMapping {
  protocol: StreamMappingProtocol;
  listen_port: number;
  target: string;
  use_auth: boolean;
}

export type PasskeyRpMode = "auth_host" | "parent_domain";
export type PostLoginIpGrantMode = "follow_session" | "disabled" | "custom";

export interface SubdomainModeConfig {
  root_domain: string;
  auth_host: string;
  auth_target: string;
  cookie_domain: string;
  edge_client_ip_enabled: boolean;
  aliyun_esa_enabled: boolean;
  tencent_edgeone_enabled: boolean;
  public_auth_base_url: string;
  public_http_port?: number;
  public_https_port?: number;
  auth_cache_ttl_seconds: number;
  auth_cache_unauthorized_ttl_seconds: number;
  default_access_mode: HostAccessMode;
  auto_add_whitelist_on_login: boolean;
  passkey_rp_mode: PasskeyRpMode;
  passkey_rp_id?: string;
}

export interface SSLConfig {
  id?: string;
  label?: string;
  source?: SSLCertificateSource;
  primary_domain?: string;
  cert: string;
  key: string;
  activate?: boolean;
}

export interface SSLCertInfo {
  issuer: string;
  subject: string;
  validFrom: string;
  validTo: string;
  dnsNames: string[];
  serialNumber: string;
}

export type SSLDeploymentMode = "single_active" | "multi_sni";
export type SSLCertificateSource = "manual" | "acme" | "ca";

export interface SubdomainCertificateCoverage {
  status: "ready" | "partial" | "missing";
  auth_host?: string;
  certificate_domains: string[];
  recommended_domains: string[];
  covered_recommended_domains: string[];
  uncovered_recommended_domains: string[];
  covered_hosts: string[];
  uncovered_hosts: string[];
  covers_auth_host: boolean;
  warnings: string[];
  summary: string;
}

export interface SubdomainCertificateLibraryCoverage {
  status: "ready" | "partial" | "missing";
  deployment_mode: SSLDeploymentMode;
  active_certificate_id?: string;
  fully_covering_certificate_ids: string[];
  partially_covering_certificate_ids: string[];
  combined_covering_certificate_ids: string[];
  suggested_certificate_id?: string;
  can_auto_activate: boolean;
  warnings: string[];
  summary: string;
}

export interface SSLCertificateSummary {
  id: string;
  label: string;
  source: SSLCertificateSource;
  primary_domain?: string;
  created_at: string;
  updated_at: string;
  certInfo?: SSLCertInfo;
  is_active: boolean;
  coverage?: SubdomainCertificateCoverage;
}

export interface SSLStatus {
  enabled: boolean;
  activeCertId?: string;
  deploymentMode: SSLDeploymentMode;
  configuredDeploymentMode?: SSLDeploymentMode;
  certInfo?: SSLCertInfo;
  certificates: SSLCertificateSummary[];
  subdomain_coverage?: SubdomainCertificateCoverage;
  library_coverage?: SubdomainCertificateLibraryCoverage;
  gateway_status?: {
    enabled: boolean;
    deployment_mode: SSLDeploymentMode;
    certificates: Array<{
      id?: string;
      label?: string;
      domains?: string[];
      is_default?: boolean;
    }>;
    sync_error?: string;
  };
}

export interface SharedDataFileEntry {
  name: string;
  relativePath: string;
  extension: string;
  size: number;
  modifiedAt: string;
}

export interface SSLSharedFilesPayload {
  shareName: string;
  available: boolean;
  files: SharedDataFileEntry[];
}

export interface FnosShareBypassConfig {
  enabled: boolean;
  upstream_timeout_ms: number;
  validation_cache_ttl_seconds: number;
  validation_lock_ttl_seconds: number;
  session_ttl_seconds: number;
}

export interface FnosPortIconHijackConfig {
  enabled: boolean;
  updated_at: string | null;
}

export type FnosCertificateSyncStatus =
  | "unmatched"
  | "up_to_date"
  | "syncable"
  | "source_invalid"
  | "target_invalid"
  | "protected"
  | "sync_failed";

export interface FnosCertificateSyncItem {
  target_id: string;
  domain: string;
  san: string[];
  source: string;
  renewal: boolean;
  valid_from: number | null;
  valid_to: number | null;
  fingerprint: string | null;
  status: FnosCertificateSyncStatus;
  reason: string | null;
  local: {
    id: string;
    label: string;
    valid_from: number | null;
    valid_to: number | null;
    fingerprint: string | null;
  } | null;
}

export interface FnosCertificateSyncDetails {
  availability: { available: boolean; reason: string | null };
  config: { auto_sync_enabled: boolean };
  runtime: {
    running: boolean;
    last_sync_at: number | null;
    last_result: FnosCertificateSyncSummary | null;
    last_error: string | null;
  };
  summary: { total: number; syncable: number; up_to_date: number };
  certificates: FnosCertificateSyncItem[];
}

export interface FnosCertificateSyncSummary {
  synced: number;
  skipped: number;
  failed: number;
  rolled_back: boolean;
}

export interface FnosCertificateSyncResponse {
  summary: FnosCertificateSyncSummary;
  details: FnosCertificateSyncDetails;
}

export interface FnosNetworkTuningConfig {
  bbr_enabled: boolean;
  mtu_probing_enabled: boolean;
  previous_tcp_congestion_control: string | null;
  previous_default_qdisc: string | null;
  previous_tcp_mtu_probing: string | null;
  updated_at: string | null;
  last_error: string | null;
}

export interface FnosNetworkTuningKernelState {
  tcp_congestion_control: string | null;
  tcp_available_congestion_control: string[];
  default_qdisc: string | null;
  tcp_mtu_probing: string | null;
  mtu_probing_supported: boolean;
  bbr_module_loaded: boolean;
  bbr_supported: boolean;
  bbr_active: boolean;
  mtu_probing_active: boolean;
}

export interface FnosNetworkTuningStatus {
  available: boolean;
  blocked_reason_code: "deployment" | "platform" | "permission" | null;
  blocked_reason: string | null;
  managed_config_path: string;
  config: FnosNetworkTuningConfig;
  state: FnosNetworkTuningKernelState;
  bbr: {
    desired_enabled: boolean;
    active: boolean;
    supported: boolean;
    module_loaded: boolean;
    current_congestion_control: string | null;
    current_default_qdisc: string | null;
    available_congestion_control: string[];
  };
  mtu_probing: {
    desired_enabled: boolean;
    active: boolean;
    supported: boolean;
    current_value: string | null;
  };
  last_error: string | null;
}

export interface FnosNetworkTuningUpdatePayload {
  bbr_enabled?: boolean;
  mtu_probing_enabled?: boolean;
}

export interface GatewayLoggingConfig {
  enabled: boolean;
  max_days: number;
  logs_dir: string;
  dropped_entries?: number;
  queue_size?: number;
  queue_depth?: number;
}

export * from "./types/waf";

export type IpLocationLookupStatus =
  | "idle"
  | "queued"
  | "processing"
  | "success"
  | "failed"
  | "skipped";

export interface IpLocationSnapshot {
  ip: string;
  normalizedIp: string;
  status: IpLocationLookupStatus;
  attempts: number;
  maxAttempts: number;
  location: string;
  error?: string;
  updatedAt: number;
}

export interface IpLocationBatchPayload {
  items: IpLocationSnapshot[];
}

export interface ProtocolMappingFeatureConfig {
  enabled: boolean;
}

export interface AutoHttpsConfig {
  enabled: boolean;
}

export type AutoHttpsRuntimeStatus = "disabled" | "active" | "error";

export interface AutoHttpsRuntimeState {
  enabled: boolean;
  active: boolean;
  status: AutoHttpsRuntimeStatus;
  listen_host: string;
  listen_port: number;
  redirect_scheme: "https";
  last_error: string | null;
  last_error_at: string | null;
  updated_at: string;
}

export interface AutoHttpsDetails extends AutoHttpsConfig {
  runtime: AutoHttpsRuntimeState;
}

export interface DashboardDisplayConfig {
  show_entry_status_module: boolean;
}

export interface SmartConnectConfig {
  enabled: boolean;
  selected_ipv4: string;
}

export interface ScanDiscoveryConfig {
  custom_cidrs: string[];
  selected_cidrs: string[];
}

export interface SmartConnectRuntimeState {
  selected_ipv4: string;
  synced_domains: string[];
  managed_rule_count: number;
  last_sync_at: string | null;
  last_sync_error: string | null;
}

export type DnsmasqInstallStatus =
  | "uninstalled"
  | "installing"
  | "installed"
  | "error";

export interface DnsmasqInstallState {
  status: DnsmasqInstallStatus;
  progress: number;
  message: string;
}

export interface DnsmasqStatus {
  installed: boolean;
  service_active: boolean;
  initialized: boolean;
  version: string;
  install_state: DnsmasqInstallState;
}

export interface SmartConnectAvailability {
  available: boolean;
  reason: string;
}

export interface SmartConnectLocalIpOption {
  label: string;
  value: string;
  interface: string;
}

export interface SmartConnectDetails {
  config: SmartConnectConfig;
  availability: SmartConnectAvailability;
  dnsmasq: DnsmasqStatus & {
    runtime: SmartConnectRuntimeState;
  };
  domains: string[];
  local_ip_options: SmartConnectLocalIpOption[];
}

export interface AuthCredentialSettings {
  session_ttl_seconds: number;
  remember_me_ttl_seconds: number;
  post_login_ip_grant_mode: PostLoginIpGrantMode;
  post_login_ip_grant_ttl_seconds: number | null;
  session_ip_mobility_enabled: boolean;
  session_ip_mobility_window_seconds: number;
  passkey_bind_prompt_enabled: boolean;
}

export interface GatewayLogEntry {
  time?: string;
  level?: string;
  method?: string;
  scheme?: string;
  host?: string;
  path?: string;
  query?: string;
  request_uri?: string;
  protocol?: string;
  status: number;
  duration_ms: number;
  client_ip?: string;
  remote_ip?: string;
  remote_addr?: string;
  user_agent?: string;
  referer?: string;
  logged_in: boolean;
  auth_required: boolean;
  auth_decision?: string;
  auth_rule_group_id?: string;
  auth_grant_state?: string;
  auth_credential_id?: string;
  auth_credential_name?: string;
  auth_credential_method?: string;
  auth_linked_totp_id?: string;
  auth_linked_totp_name?: string;
  access_mode?: string;
  route_type?: string;
  route_key?: string;
  upstream?: string;
  matched: boolean;
  bytes_in: number;
  bytes_out: number;
  tls: boolean;
  websocket: boolean;
  ali_real_client_ip?: string;
  eo_connecting_ip?: string;
  x_forwarded_for?: string;
  x_real_ip?: string;
  waf_blocked?: boolean;
  waf_trace_id?: string;
  waf_mode?: string;
  waf_rule_ids?: number[];
  waf_action?: string;
  waf_bundle?: string;
  general_blacklist_blocked?: boolean;
  ipLocation?: string;
}

export interface GatewayLogDatesPayload {
  today: string;
  logs_dir: string;
  dates: string[];
}

export interface GatewayLogEntriesPayload {
  date: string;
  logs_dir: string;
  available_dates: string[];
  pagination: "page" | "cursor";
  page: number;
  limit: number;
  total: number;
  cursor?: string;
  next_cursor?: string;
  has_more: boolean;
  items: GatewayLogEntry[];
}

export interface GatewayLogDeletePayload {
  date: string;
  logs_dir: string;
  deleted: boolean;
  available_dates: string[];
}

export interface FnKnockBackupImportArchiveRequest {
  filename?: string;
  archive_base64: string;
}

export interface FnKnockBackupImportResult {
  cleared_keys: number;
  imported_keys: number;
  warnings: string[];
  synced_steps: string[];
}

export interface BackupDirectoryFilesPayload {
  shareName: string;
  available: boolean;
  files: SharedDataFileEntry[];
}

export interface FnKnockBackupExportToDirectoryResult {
  filename: string;
  relativePath: string;
  filePath: string;
  size: number;
  exportedAt: string;
}

export interface TerminalFeatureConfig {
  enabled: boolean;
  default_cwd: string;
  max_sessions: number;
  idle_timeout_seconds: number;
  resume_backend: "tmux";
  allow_mobile_toolbar: boolean;
  dangerously_run_as_current_user: boolean;
}

export type TerminalTmuxDetectionSource = "env-path" | "absolute-path";
export type TerminalTmuxInstallStatus =
  | "uninstalled"
  | "installing"
  | "installed"
  | "error";

export interface TerminalTmuxInstallState {
  status: TerminalTmuxInstallStatus;
  progress: number;
  message: string;
  executablePath: string;
  detectionSource: TerminalTmuxDetectionSource | null;
  version: string;
}

export type TerminalTransport = "http-polling";
export type TerminalSessionStatus =
  | "created"
  | "attached"
  | "detached"
  | "stopped"
  | "error";

export interface TerminalSessionRecord {
  id: string;
  title: string;
  status: TerminalSessionStatus;
  created_at: string;
  updated_at: string;
  last_attached_at: string;
  last_detached_at: string;
  last_client_ip: string;
  shell: string;
  cwd: string;
  cols: number;
  rows: number;
  resume_backend: "tmux";
  backend_session_name: string;
  pane_tty_path: string;
  input_pipe_path: string;
  output_log_path: string;
  expires_at: string;
  last_frame_revision?: string;
}

export interface TerminalAttachmentRecord {
  id: string;
  session_id: string;
  transport: TerminalTransport;
  created_at: string;
  updated_at: string;
  expires_at: string;
}

export interface TerminalOutputChunk {
  cursor: number;
  data_base64: string;
  reset: boolean;
  updatedAt: string;
}

export interface TerminalRuntimeStatus {
  enabled: boolean;
  tmuxAvailable: boolean;
  tmuxExecutablePath: string;
  tmuxDetectionSource: TerminalTmuxDetectionSource | null;
  tmuxVersion: string;
  tmuxInstallState: TerminalTmuxInstallState;
  httpPollingAvailable: boolean;
  runningAsRoot: boolean;
  blockedReason: string;
}

export interface AppConfig {
  run_type: RunType;
  reverse_proxy_submode: ReverseProxySubmode;
  auto_manage_firewall: boolean;
  runtime_profile?: RuntimeProfile;
  capabilities?: RuntimeCapabilities;
  whitelist_ips: string[];
  default_route: string;
  proxy_mappings: ProxyMapping[];
  host_mappings: HostMapping[];
  stream_mappings: StreamMapping[];
  subdomain_mode: SubdomainModeConfig;
  default_tunnel?: "frp" | "cloudflared";
  fnos_share_bypass?: FnosShareBypassConfig;
  fnos_port_icon_hijack?: FnosPortIconHijackConfig;
  fnos_network_tuning?: FnosNetworkTuningConfig;
  fnos_certificate_sync?: { auto_sync_enabled: boolean };
  gateway_logging?: GatewayLoggingConfig;
  waf?: WAFConfig;
  reverse_proxy_throttle?: ReverseProxyThrottleConfig;
  gateway_proxy_headers?: GatewayProxyHeadersConfig;
  gateway_host_response?: GatewayHostResponseConfig;
  gateway_crawler_blocker?: GatewayCrawlerBlockerConfig;
  gateway_portal?: GatewayPortalConfig;
  appearance?: AppearanceConfig;
  protocol_mapping_feature?: ProtocolMappingFeatureConfig;
  auto_https?: AutoHttpsConfig;
  dashboard_display?: DashboardDisplayConfig;
  smart_connect?: SmartConnectConfig;
  scan_discovery?: ScanDiscoveryConfig;
  auth_credential_settings?: AuthCredentialSettings;
  terminal_feature?: TerminalFeatureConfig;
  ssh_security?: SSHSecurityConfig;
  locale?: LocaleConfig;
  ssl: {
    enabled: boolean;
    active_cert_id?: string;
    deployment_mode?: SSLDeploymentMode;
    certificate_count?: number;
  };
  login: {
    nonce_list: string[];
    ip_backoff: Record<string, number>;
  };
}

export type TOTPAccessScope = "docker_admin_panel";
export type TOTPSubdomainAccessMode = "all" | "custom";

export type TOTPSubdomainAccess = {
  mode: TOTPSubdomainAccessMode;
  hosts: string[];
};

export type TOTPCredential = {
  id: string;
  secret: string;
  comment: string;
  createdAt: string;
  access_scopes: TOTPAccessScope[];
  subdomain_access: TOTPSubdomainAccess;
};

export type TOTPCredentialImportSummary = {
  kind?: "totp" | "password";
  login_mode?: AuthLoginMode;
  imported: number;
  skipped_existing_id: number;
  skipped_existing_secret: number;
  skipped_existing_username?: number;
  skipped_file_duplicate: number;
  invalid: number;
  total: number;
  password_total?: number;
  password_imported?: number;
  password_skipped_existing?: number;
  password_skipped_missing_account?: number;
  password_skipped_file_duplicate?: number;
  password_invalid?: number;
  totp_total?: number;
  totp_imported?: number;
  totp_skipped_existing_id?: number;
  totp_skipped_existing_secret?: number;
  totp_skipped_file_duplicate?: number;
  totp_invalid?: number;
};

export type AuthLoginMode = "totp" | "password";

export type AuthLoginModeStatus = {
  mode: AuthLoginMode;
  totpCount: number;
  accountCount: number;
  passwordConfiguredCount: number;
  passwordMissingCount: number;
};

export type AuthLoginModePreview = {
  currentMode: AuthLoginMode;
  targetMode: AuthLoginMode;
  totpCount: number;
  accountCount: number;
  createAccountCount: number;
  updateAccountCount: number;
  passwordConfiguredCount: number;
  passwordMissingCount: number;
  blockingIssueCount: number;
  passwordRequiredBeforeSwitch?: boolean;
  missingSourceTotpCount?: number;
};

export type AuthAccount = {
  id: string;
  username: string;
  displayName: string;
  sourceTotpId: string;
  sourceTotpName: string;
  createdAt: string;
  updatedAt: string;
  access_scopes: TOTPAccessScope[];
  subdomain_access: TOTPSubdomainAccess;
  passwordConfigured: boolean;
  totpConfigured: boolean;
};

export type PasskeyCredential = {
  id: string;
  totpId: string;
  publicKey: string;
  counter: number;
  transports?: string[];
  deviceName: string;
  createdAt: string;
  lastUsedAt?: string;
};

export type ExternalAuthProviderType =
  | "fnknock_qq"
  | "google"
  | "microsoft"
  | "github"
  | "custom_oidc";

export type ExternalAuthProtocol = "oidc" | "oauth2_profile";

export type OIDCProviderCatalogItem = {
  type: ExternalAuthProviderType;
  protocol: ExternalAuthProtocol;
  label: string;
  description: string;
  default_name: string;
  default_scopes: string[];
  required_fields: string[];
  optional_fields: string[];
  supports_pkce: boolean;
  supports_discovery: boolean;
};

export type OIDCProviderView = {
  id: string;
  type: ExternalAuthProviderType;
  protocol: ExternalAuthProtocol;
  name: string;
  enabled: boolean;
  connection_config_masked: Record<string, unknown>;
  callback_url?: string;
  created_at: string;
  updated_at: string;
  last_test_at?: string;
  last_test_status?: "idle" | "success" | "failed";
  last_error?: string | null;
};

export type OIDCBinding = {
  id: string;
  provider_id: string;
  provider_type: ExternalAuthProviderType;
  provider_name?: string;
  totp_id: string;
  totp_name?: string;
  issuer: string;
  subject: string;
  display_name?: string;
  email?: string;
  email_verified?: boolean;
  avatar_url?: string;
  created_at: string;
  updated_at: string;
  last_used_at?: string;
};

export type LoginSession = {
  totpId: string;
  method: "TOTP" | "PASSKEY" | "OIDC";
  credentialId: string;
  credentialName: string;
  comment?: string;
  ip: string;
  userAgent: string;
  loginTime: string;
  expiresAt?: string;
  ipLocation?: string;
};

export type SessionMobilitySummary = {
  hasHistory: boolean;
  driftCount: number;
  lastDriftAt: string | null;
  lastDriftSource:
    | "proxy-session"
    | "fnos-token"
    | "session-refresh"
    | "browser-session"
    | null;
};

export type SessionMobilityEvent =
  | {
      version: 1;
      kind: "login";
      happenedAt: string;
      source: "login";
      toIp: string;
      toIpLocation?: string;
    }
  | {
      version: 1;
      kind: "drift";
      happenedAt: string;
      source:
        | "proxy-session"
        | "fnos-token"
        | "session-refresh"
        | "browser-session";
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

export type SessionMobilityDetails = {
  summary: SessionMobilitySummary;
  events: SessionMobilityEvent[];
};

export type SessionAppAttachmentRecord = {
  subjectHash: string;
  currentIp: string;
  createdAt: string;
  lastSeenAt: string;
  expiresAt: string | null;
};

export type SessionFnosAttachmentRecord = SessionAppAttachmentRecord;
export type SessionTrimMediaAttachmentRecord = SessionAppAttachmentRecord;

export type SessionRecord = LoginSession & {
  id: string;
  mobility?: SessionMobilitySummary;
  fnosAttachments?: SessionFnosAttachmentRecord[];
  trimMediaAttachments?: SessionTrimMediaAttachmentRecord[];
};

export type ProxyProtocolForce = {
  proxy_protocol_force: boolean;
};

export type ReverseProxyThrottleConfig = {
  enabled: boolean;
  requests_per_second: number;
  burst: number;
  block_seconds: number;
};

export type GatewayVisibilitySelection = {
  province: string;
  city: string | null;
  label: string;
  value: string;
  query_city: string | null;
  operator?: import("./types/cidr").CidrOperator | null;
  is_province_wide: boolean;
  is_municipality: boolean;
};

export type GatewayVisibilitySummary = {
  enabled: boolean;
  selection_count: number;
  custom_cidr_count: number;
  cidr_count: number;
  updated_at: string | null;
};

export type GatewayVisibilityConfig = {
  enabled: boolean;
  selections: GatewayVisibilitySelection[];
  custom_cidrs: string[];
};

export type GatewayVisibilityDetails = {
  config: GatewayVisibilityConfig;
  summary: GatewayVisibilitySummary;
};

export type SSHSecurityBlockDurationUnit = "minute" | "hour" | "day";

export type SSHSecuritySelection = GatewayVisibilitySelection;

export type SSHSecurityConfig = {
  enabled: boolean;
  window_minutes: number;
  failed_login_threshold: number;
  block_duration_value: number;
  block_duration_unit: SSHSecurityBlockDurationUnit;
  allowed_regions: SSHSecuritySelection[];
  custom_cidrs: string[];
  configured_at: string | null;
  updated_at: string | null;
};

export type SSHSecuritySummary = {
  configured: boolean;
  enabled: boolean;
  allowed_cidr_count: number;
  active_block_count: number;
  ssh_ports: number[];
  log_source: "journal" | "auth.log" | "unavailable";
  available: boolean;
  unavailable_reason: string;
  updated_at: string | null;
};

export type SSHSecurityDetails = {
  config: SSHSecurityConfig;
  summary: SSHSecuritySummary;
};

export type SSHLoginLogEntry = {
  id: string;
  happened_at: string;
  outcome: "success" | "failure";
  username: string;
  invalid_user: boolean;
  ip: string;
  ipLocation?: string;
  port?: number;
  related_ports?: number[];
  repeat_count?: number;
  auth_method?: string;
  service: "sshd";
  source: "journal" | "auth.log";
  raw: string;
};

export type SSHLoginLogListPayload = {
  items: SSHLoginLogEntry[];
  total: number;
  page: number;
  limit: number;
};

export type SSHSecurityBlockReason =
  | "failed_login_threshold"
  | "cidr_not_allowed";

export type SSHSecurityBlockRecord = {
  ip: string;
  ipLocation?: string;
  ports?: number[];
  blocked_at: string;
  expires_at: string;
  reason: SSHSecurityBlockReason;
  failed_count: number;
  window_minutes: number;
  threshold: number;
  sample_user?: string;
  sample_auth_method?: string;
  sample_log_time?: string;
  applied: boolean;
  removed_at?: string | null;
  remove_reason?: "manual" | "expired" | "disabled" | null;
};

export type SSHSecurityBlockListPayload = {
  items: SSHSecurityBlockRecord[];
  total: number;
  page: number;
  limit: number;
};

export type SSHSecurityFirewallSyncResult = {
  cleared: number;
  synced: number;
  active_blocks: number;
  allowed_cidrs: number;
  ports: number[];
};

export type SSHSecurityFirewallClearResult = {
  cleared_blocks: number;
};

export type GatewayProxyHeadersConfig = {
  disabled_hosts: string[];
};

export type GatewayProxyHeadersItem = {
  host: string;
  target: string;
  title: string;
  send_proxy_headers: boolean;
};

export type GatewayProxyHeadersAvailability = {
  available: boolean;
  reason: string;
};

export type GatewayProxyHeadersSummary = {
  total_count: number;
  disabled_count: number;
  updated_at: string | null;
};

export type GatewayProxyHeadersDetails = {
  config: GatewayProxyHeadersConfig;
  availability: GatewayProxyHeadersAvailability;
  items: GatewayProxyHeadersItem[];
  summary: GatewayProxyHeadersSummary;
};

export type GatewayHostResponseConfig = {
  disabled_hosts: string[];
};

export type GatewayHostResponseItem = {
  host: string;
  target: string;
  title: string;
  preserve_host: boolean;
};

export type GatewayHostResponseAvailability = {
  available: boolean;
  reason: string;
};

export type GatewayHostResponseSummary = {
  total_count: number;
  disabled_count: number;
  updated_at: string | null;
};

export type GatewayHostResponseDetails = {
  config: GatewayHostResponseConfig;
  availability: GatewayHostResponseAvailability;
  items: GatewayHostResponseItem[];
  summary: GatewayHostResponseSummary;
};

export type GatewayPortalDisplayStyle = "domain" | "title";
export type GatewayPortalIconDragMode = "corners" | "free";

export type GatewayPortalConfig = {
  enabled: boolean;
  display_style: GatewayPortalDisplayStyle;
  show_app_icon: boolean;
  icon_drag_mode: GatewayPortalIconDragMode;
};

export type GatewayCrawlerBlockerConfig = {
  enabled: boolean;
  updated_at?: string | null;
};

export type GatewaySettings = {
  auth_cache_ttl_seconds: number;
  auth_cache_unauthorized_ttl_seconds: number;
  reverse_proxy_throttle: ReverseProxyThrottleConfig;
  visibility: GatewayVisibilitySummary;
  proxy_headers: GatewayProxyHeadersSummary;
  host_response: GatewayHostResponseSummary;
  crawler_blocker: GatewayCrawlerBlockerConfig;
  portal: GatewayPortalConfig;
};

export type TrafficStats = {
  total_in: number;
  total_out: number;
  active_conns: number;
  error_5xx: number;
  by_host?: HostTrafficStats[];
  timestamp: number;
};

export type HostTrafficStats = {
  host: string;
  total_in: number;
  total_out: number;
  error_5xx: number;
  active_ip_count?: number;
};

export type HostActiveIp = {
  ip: string;
  last_seen_at: string;
  active_conns: number;
};

export type HostActiveIpsPayload = {
  host: string;
  window_seconds: number;
  items: HostActiveIp[];
  timestamp?: number;
};

export type DashboardStats = {
  rangeSec: number;
  now: {
    online: number | null;
    error5xxTotal: number | null;
  };
  totals: {
    inBytes: number;
    outBytes: number;
    error5xx: number;
  };
  errors: {
    error5xx1d: number;
    error5xx1w: number;
  };
  traffic: {
    echarts: unknown;
  };
};

export type ThreatOverview = {
  rangeSec: number;
  totals: {
    failedLogins: number;
    blockedScanners: number;
    wafEvents: number;
  };
  series: {
    failedLogins: Array<[number, number]>;
    blockedScanners: Array<[number, number]>;
    wafEvents: Array<[number, number]>;
  };
};

export * from "./types/system-events";
export * from "./types/cidr";
