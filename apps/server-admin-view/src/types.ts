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

export interface WelcomeGuideStatus {
  completed: boolean;
  completed_at: string | null;
}

export type DeploymentTarget = "fpk" | "docker" | "dev";

export interface RuntimeProfile {
  deployment_target: DeploymentTarget;
  is_docker: boolean;
  is_linux: boolean;
  is_root_process: boolean;
}

export interface RuntimeCapabilities {
  direct_mode_available: boolean;
  host_firewall_available: boolean;
  smart_connect_available: boolean;
  system_clock_sync_available: boolean;
  self_update_available: boolean;
  terminal_available: boolean;
  shared_root_available: boolean;
}

export interface DockerAdminBootstrapState {
  enabled: boolean;
  password_configured: boolean;
  authenticated: boolean;
  session_expires_at: string | null;
}

export type HostAccessMode = "login_first" | "strict_whitelist";
export type HostServiceRole = "app" | "auth";
export type StreamMappingProtocol = "tcp" | "udp";

export interface HostMappingBasicAuth {
  enabled: boolean;
  username: string;
  password: string;
}

export interface HostMapping {
  host: string;
  target: string;
  use_auth: boolean;
  access_mode: HostAccessMode;
  suppress_toolbar: boolean;
  preserve_host: boolean;
  basic_auth: HostMappingBasicAuth;
  service_role: HostServiceRole;
  title: string;
  title_override: string;
  favicon: string;
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

export interface GatewayLoggingConfig {
  enabled: boolean;
  max_days: number;
  logs_dir: string;
}

export type WAFMode = "off" | "detection" | "blocking";

export interface WAFConfig {
  enabled: boolean;
  system_rules_auto_update_enabled: boolean;
  common_location_exempt_enabled: boolean;
  mode: WAFMode;
  active_bundle_id: string;
  rules_dir: string;
  paranoia_level: number;
  executing_paranoia_level: number;
  inbound_anomaly_threshold: number;
  outbound_anomaly_threshold: number;
  request_body_access: boolean;
  request_body_limit_bytes: number;
  request_body_in_memory_limit_bytes: number;
  response_body_access: boolean;
  disabled_hosts: string[];
  disabled_path_prefixes: string[];
  log_retention_days: number;
  drain_interval_seconds: number;
  updated_at: string | null;
}

export interface WAFStatus {
  enabled: boolean;
  mode: WAFMode | string;
  loaded: boolean;
  bundle_id?: string;
  bundle_hash?: string;
  loaded_at?: string;
  rules_dir?: string;
  pending_events: number;
  last_error?: string;
}

export interface WAFValidationResult {
  ok: boolean;
  bundle_id?: string;
  bundle_path?: string;
  bundle_hash?: string;
  error?: string;
}

export type WAFRuleSource = "system" | "custom";

export interface WAFManifestRule {
  filename: string;
  description: string;
}

export interface WAFRemoteManifest {
  rulesDescription?: {
    rules?: WAFManifestRule[];
  };
  packagingTime?: string;
  zipFile: string;
  zipHash: string;
  commitHash?: string;
  commitDate?: string;
}

export interface WAFRuleFile {
  source: WAFRuleSource;
  filename: string;
  description: string;
  enabled: boolean;
  size_bytes: number;
  updated_at: string;
}

export interface WAFRuleFileContent extends WAFRuleFile {
  content: string;
}

export interface WAFSystemSyncState {
  zip_file: string;
  zip_hash: string;
  synced_at: string;
  packaging_time?: string;
  commit_hash?: string;
  commit_date?: string;
}

export interface WAFDetails {
  config: WAFConfig;
  status: WAFStatus | null;
  rules_dir: string;
  system: {
    manifest: WAFRemoteManifest | null;
    manifest_cached_at: string | null;
    manifest_last_checked_at: string | null;
    manifest_last_error: string | null;
    synced: WAFSystemSyncState | null;
    update_available: boolean;
    rules: WAFRuleFile[];
  };
  custom: {
    rules: WAFRuleFile[];
  };
}

export interface WAFMatchedVariable {
  variable?: string;
  key?: string;
  value_preview?: string;
}

export interface WAFRuleMatch {
  id: number;
  message?: string;
  data?: string;
  severity?: string;
  phase?: number;
  file?: string;
  line?: number;
  tags?: string[];
  disruptive: boolean;
  matched_variables?: WAFMatchedVariable[];
}

export interface WAFInterruptionInfo {
  rule_id?: number;
  action?: string;
  status?: number;
}

export interface WAFEvent {
  trace_id: string;
  transaction_id?: string;
  time: string;
  mode: WAFMode | string;
  action: string;
  status?: number;
  client_ip?: string;
  remote_addr?: string;
  method?: string;
  scheme?: string;
  host?: string;
  path?: string;
  query?: string;
  request_uri?: string;
  user_agent?: string;
  referer?: string;
  route_type?: string;
  route_key?: string;
  upstream?: string;
  bundle_id?: string;
  bundle_hash?: string;
  rule_ids?: number[];
  rules?: WAFRuleMatch[];
  interruption?: WAFInterruptionInfo;
  error?: string;
}

export interface WAFDrainResult {
  events: WAFEvent[];
  drained: number;
  remaining: number;
}

export interface WAFLogEntriesPayload {
  date: string;
  available_dates: string[];
  cursor: string;
  next_cursor: string;
  has_more: boolean;
  limit: number;
  total: number;
  items: WAFEvent[];
}

export interface WAFLogDeletePayload {
  date: string;
  deleted: boolean;
  available_dates: string[];
}

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
  gateway_logging?: GatewayLoggingConfig;
  waf?: WAFConfig;
  reverse_proxy_throttle?: ReverseProxyThrottleConfig;
  gateway_proxy_headers?: GatewayProxyHeadersConfig;
  gateway_host_response?: GatewayHostResponseConfig;
  protocol_mapping_feature?: ProtocolMappingFeatureConfig;
  auto_https?: AutoHttpsConfig;
  dashboard_display?: DashboardDisplayConfig;
  smart_connect?: SmartConnectConfig;
  scan_discovery?: ScanDiscoveryConfig;
  auth_credential_settings?: AuthCredentialSettings;
  terminal_feature?: TerminalFeatureConfig;
  ssh_security?: SSHSecurityConfig;
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

export type TOTPCredential = {
  id: string;
  secret: string;
  comment: string;
  createdAt: string;
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

export type GatewaySettings = {
  auth_cache_ttl_seconds: number;
  auth_cache_unauthorized_ttl_seconds: number;
  reverse_proxy_throttle: ReverseProxyThrottleConfig;
  visibility: GatewayVisibilitySummary;
  proxy_headers: GatewayProxyHeadersSummary;
  host_response: GatewayHostResponseSummary;
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

export type SystemEventType =
  | "FN_EVENT_AUTH_LOGIN_SUCCESS"
  | "FN_EVENT_AUTH_LOGOUT"
  | "FN_EVENT_AUTH_LOGIN_FAILURE"
  | "FN_EVENT_AUTH_SESSION_IP_DRIFT"
  | "FN_EVENT_SECURITY_SCANNER_BLOCKED"
  | "FN_EVENT_DDNS_UPDATE_COMPLETED"
  | "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
  | "FN_EVENT_WAF_BLOCKED"
  | "FN_EVENT_SSH_LOGIN_SUCCESS"
  | "FN_EVENT_SSH_LOGIN_FAILURE"
  | "FN_EVENT_SSH_IP_BLOCKED"
  | "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE"
  | "FN_EVENT_SYSTEM_CPU_ALERT"
  | "FN_EVENT_SYSTEM_CPU_RECOVERED"
  | "FN_EVENT_SYSTEM_MEMORY_ALERT"
  | "FN_EVENT_SYSTEM_MEMORY_RECOVERED"
  | "FN_EVENT_TUNNEL_FRP_CONNECTED"
  | "FN_EVENT_TUNNEL_FRP_DISCONNECTED"
  | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
  | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED";

export type SystemEventLevel = "INFO" | "WARN" | "ERROR" | "CRITICAL";

export type SystemEventSource =
  | "SERVER_ADMIN"
  | "GO_REAUTH_PROXY"
  | "SYSTEM_MONITOR";

export type SystemEventSubjectKind =
  | "IP"
  | "SESSION"
  | "DDNS"
  | "RESOURCE"
  | "APPLICATION"
  | "TUNNEL";

export interface SystemEventSubject {
  kind: SystemEventSubjectKind;
  id: string;
}

export interface SystemEventRecord {
  id: string;
  type: SystemEventType;
  source: SystemEventSource;
  level: SystemEventLevel;
  happened_at: string;
  dedupe_key?: string;
  subject?: SystemEventSubject;
  tags?: string[];
  payload: Record<string, unknown>;
}

export interface SystemEventListPayload {
  events: SystemEventRecord[];
  total: number;
}

export type NotificationProviderType =
  | "wxpusher"
  | "serverchan"
  | "pushplus"
  | "wecom"
  | "dingtalk"
  | "feishu"
  | "email"
  | "webhook"
  | "pushdeer"
  | "magicpush"
  | "bark"
  | "telegram";

export type NotificationGroupBy =
  | "GLOBAL"
  | "IP"
  | "SESSION"
  | "SUBJECT"
  | "HOSTNAME"
  | "PROVIDER";

export type NotificationTriggerStatus =
  | "created"
  | "fanout_done"
  | "partially_failed"
  | "completed";

export type NotificationDeliveryStatus =
  | "queued"
  | "sending"
  | "success"
  | "failed"
  | "gave_up"
  | "skipped";

export type NotificationTestStatus = "idle" | "success" | "failed";

export type NotificationMessageTemplateMode = "default" | "custom";

export type NotificationTemplateOverrideMode = "inherit" | "custom";

export type NotificationSeverity = "info" | "warn" | "error" | "critical";

export type NotificationFieldType =
  | "string"
  | "number"
  | "boolean"
  | "select"
  | "json";

export interface NotificationFieldOption {
  label: string;
  value: string;
}

export interface NotificationSchemaField {
  key: string;
  label: string;
  description?: string;
  placeholder?: string;
  type: NotificationFieldType;
  required?: boolean;
  sensitive?: boolean;
  default_value?: string | number | boolean | null;
  options?: NotificationFieldOption[];
  min?: number;
  max?: number;
}

export interface NotificationProviderCapabilities {
  supports_text: boolean;
  supports_markdown: boolean;
  supports_rich_blocks: boolean;
  supports_actions: boolean;
  supports_mentions: boolean;
  supports_attachments: boolean;
  supports_provider_dedupe_key: boolean;
  max_body_length?: number | null;
}

export interface NotificationProviderDefinition {
  type: NotificationProviderType;
  label: string;
  description: string;
  connection_schema: NotificationSchemaField[];
  target_schema: NotificationSchemaField[];
  sensitive_fields: string[];
  capabilities: NotificationProviderCapabilities;
}

export interface NotificationMessageFact {
  label: string;
  value: string;
}

export interface NotificationMessageAction {
  label: string;
  url: string;
}

export interface NotificationMessage {
  title: string;
  summary: string;
  body_text: string;
  body_markdown?: string;
  severity: NotificationSeverity;
  facts: NotificationMessageFact[];
  actions: NotificationMessageAction[];
  mentions: string[];
  dedupe_key?: string;
  occurred_at: string;
  event_id?: string;
  metadata?: Record<string, unknown>;
}

export interface NotificationTemplate {
  title?: string;
  body_text?: string;
  body_markdown?: string;
}

export interface NotificationDeliveryPolicy {
  timeout_seconds?: number;
  max_attempts?: number;
  backoff_seconds?: number;
}

export interface NotificationProviderView {
  id: string;
  name: string;
  type: NotificationProviderType;
  enabled: boolean;
  connection_config_masked: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  last_test_at?: string;
  last_test_status?: NotificationTestStatus;
  last_error?: string | null;
}

export interface NotificationProviderDetailView extends NotificationProviderView {
  connection_config: Record<string, unknown>;
}

export interface NotificationTargetBinding {
  id: string;
  provider_id: string;
  enabled: boolean;
  target_config: Record<string, unknown>;
  template_override_mode: NotificationTemplateOverrideMode;
  template_override?: NotificationTemplate | null;
  delivery_policy?: NotificationDeliveryPolicy | null;
  created_at: string;
  updated_at: string;
}

export interface NotificationRule {
  id: string;
  name: string;
  enabled: boolean;
  event_type: SystemEventType;
  event_level_filter?: SystemEventLevel[];
  event_source_filter?: SystemEventSource[];
  window_seconds: number;
  threshold_count: number;
  group_by: NotificationGroupBy;
  cooldown_seconds: number;
  targets: NotificationTargetBinding[];
  message_template_mode: NotificationMessageTemplateMode;
  message_template?: NotificationTemplate | null;
  created_at: string;
  updated_at: string;
  last_triggered_at?: string | null;
}

export interface NotificationTrigger {
  id: string;
  rule_id: string;
  event_id: string;
  group_key: string;
  matched_count: number;
  message_snapshot: NotificationMessage;
  rule_snapshot: NotificationRule;
  status: NotificationTriggerStatus;
  created_at: string;
}

export interface NotificationDelivery {
  id: string;
  trigger_id: string;
  rule_id: string;
  target_id: string;
  provider_id: string;
  event_id: string;
  status: NotificationDeliveryStatus;
  reason?: string | null;
  provider_type: NotificationProviderType;
  message_snapshot: NotificationMessage;
  target_snapshot: NotificationTargetBinding;
  provider_snapshot: NotificationProviderView;
  request_summary?: Record<string, unknown> | null;
  response_summary?: Record<string, unknown> | null;
  attempt_count: number;
  triggered_at: string;
  sent_at?: string | null;
  next_retry_at?: string | null;
}

export interface NotificationProviderCatalogPayload {
  providers: NotificationProviderDefinition[];
}

export interface NotificationProviderListPayload {
  providers: NotificationProviderView[];
}

export interface NotificationRuleListPayload {
  rules: NotificationRule[];
}

export interface NotificationTriggerListPayload {
  triggers: NotificationTrigger[];
  total: number;
}

export interface NotificationDeliveryListPayload {
  deliveries: NotificationDelivery[];
  total: number;
}

export const CIDR_PROVINCE_WIDE_VALUE = "__province_all__";

export interface CidrProvinceItem {
  name: string;
  cityCount: number;
  isMunicipality: boolean;
  hasChildren: boolean;
}

export interface CidrProvinceOption {
  label: string;
  value: string;
  cityCount: number;
  isMunicipality: boolean;
}

export interface CidrProvincesPayload {
  items: CidrProvinceItem[];
  options: CidrProvinceOption[];
  total: number;
}

export interface CidrCityItem {
  name: string;
  ipv4Count: number;
  ipv6Count: number;
}

export interface CidrCityOption {
  label: string;
  value: string;
  queryCity: string | null;
  isProvinceWide: boolean;
  isMunicipality: boolean;
  ipv4Count: number;
  ipv6Count: number;
}

export interface CidrCitiesPayload {
  province: string;
  items: CidrCityItem[];
  options: CidrCityOption[];
  total: number;
  isMunicipality: boolean;
  supportsProvinceWide: boolean;
  defaultValue: string;
}

export interface CidrSelectorPayload {
  provinces: CidrProvincesPayload;
  cities: CidrCitiesPayload | null;
}

export interface CidrSelectionPayload {
  province: string;
  city: string | null;
  label: string;
  value: string;
  queryCity: string | null;
  isProvinceWide: boolean;
  isMunicipality: boolean;
}

export interface CidrLookupPayload {
  province: string;
  city: string | null;
  selection: CidrSelectionPayload;
  cidrGroups: {
    ipv4: string[];
    ipv6: string[];
  };
  counts: {
    ipv4: number;
    ipv6: number;
  };
  totalCount: number;
}
