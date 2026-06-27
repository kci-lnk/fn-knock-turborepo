export interface GoResponse<T = unknown> {
  success: boolean;
  code?: number;
  message?: string;
  data?: T;
  timestamp?: number;
}

export interface AuthConfig {
  auth_port?: number;
  auth_url?: string;
  login_url?: string;
  logout_url?: string;
  preflight_url?: string;
  auth_cache_ttl_seconds?: number;
  auth_cache_unauthorized_ttl_seconds?: number;
  edge_client_ip_enabled?: boolean;
  aliyun_esa_enabled?: boolean;
  tencent_edgeone_enabled?: boolean;
  public_auth_base_url?: string;
  public_http_port?: number;
  public_https_port?: number;
  auth_host?: string;
  trust_forwarded_proto?: boolean;
}

export type LocaleCode = "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP";

export interface LocaleConfig {
  default_locale: LocaleCode;
}

export interface Rule {
  path: string;
  target: string;
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
}

export interface HostRule {
  host: string;
  target: string;
  use_auth: boolean;
  access_mode?: "login_first" | "strict_whitelist";
  suppress_toolbar?: boolean;
  preserve_host?: boolean;
  title?: string;
  title_override?: string;
  favicon?: string | null;
  basic_auth?: {
    enabled: boolean;
    username: string;
    password: string;
  };
  locations?: HostLocation[];
}

export interface HostLocation {
  path: string;
  match: "exact" | "prefix";
  action: "proxy" | "response";
  target?: string;
  strip_path?: boolean;
  rewrite_html?: boolean;
  response?: {
    status: number;
    content_type: string;
    headers: Record<string, string>;
    body: string;
  };
}

export type StreamMappingProtocol = "tcp" | "udp";

export interface StreamRule {
  protocol?: StreamMappingProtocol;
  listen_port: number;
  target: string;
  use_auth: boolean;
}

export interface SSLRequest {
  cert: string;
  key: string;
}

export type SSLDeploymentMode = "single_active" | "multi_sni";

export interface SSLDeployedCertificate {
  id?: string;
  label?: string;
  cert: string;
  key: string;
  is_default?: boolean;
}

export interface SSLDeployedCertificateInfo {
  id?: string;
  label?: string;
  domains?: string[];
  is_default?: boolean;
}

export interface SSLDeploymentRequest {
  deployment_mode?: SSLDeploymentMode;
  certificates?: SSLDeployedCertificate[];
  cert?: string;
  key?: string;
}

export interface SSLInfo {
  enabled: boolean;
  deployment_mode?: SSLDeploymentMode;
  certificates?: SSLDeployedCertificateInfo[];
}

export interface ServerInfo {
  version: string;
}

export interface ProxyProtocolForceRequest {
  proxy_protocol_force: boolean;
}

export interface ProxyProtocolForceResponse {
  proxy_protocol_force: boolean;
}

export interface TrafficStats {
  total_in: number;
  total_out: number;
  active_conns: number;
  error_5xx: number;
  by_host?: HostTrafficStats[];
}

export interface HostTrafficStats {
  host: string;
  total_in: number;
  total_out: number;
  error_5xx: number;
  active_ip_count?: number;
}

export interface HostActiveIPStats {
  ip: string;
  last_seen_at: string;
  active_conns: number;
}

export interface HostActiveIPsStats {
  host: string;
  window_seconds: number;
  items: HostActiveIPStats[];
}

export interface GatewayLoggingConfig {
  enabled: boolean;
  max_days: number;
  logs_dir?: string;
}

export interface ReverseProxyThrottleConfig {
  enabled: boolean;
  requests_per_second: number;
  burst: number;
  block_seconds: number;
}

export interface GatewayVisibilityConfig {
  enabled: boolean;
  cidrs: string[];
  updated_at?: string | null;
}

export interface ForwardedHeadersConfig {
  enabled: boolean;
  omit_targets: string[];
  updated_at?: string | null;
}

export interface PreserveHostConfig {
  enabled: boolean;
  omit_targets: string[];
  updated_at?: string | null;
}

export interface CrawlerBlockerConfig {
  enabled: boolean;
  updated_at?: string | null;
}

export type GatewayPortalDisplayStyle = "domain" | "title";

export interface GatewayPortalConfig {
  enabled: boolean;
  display_style: GatewayPortalDisplayStyle;
  show_app_icon?: boolean;
}

export interface FnosPortIconHijackConfig {
  enabled: boolean;
  updated_at?: string | null;
}

export interface ReverseProxyThrottleExemptIPsRuntime {
  enabled: boolean;
  ips: string[];
  cidrs?: string[];
  updated_at?: string | null;
}

export interface CommonLocationExemptionsRuntime {
  enabled: boolean;
  waf_enabled: boolean;
  cidrs: string[];
  updated_at?: string | null;
}

export interface GatewayLoggingDirectory {
  logs_dir: string;
}

export interface GatewayLogDates {
  today: string;
  logs_dir: string;
  dates: string[];
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
}

export interface GatewayLogEntriesResponse {
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

export interface GatewayLogDeleteResponse {
  date: string;
  logs_dir: string;
  deleted: boolean;
  available_dates: string[];
}

export type GeneralBlacklistSource =
  | "manual"
  | "request_log"
  | "active_ip"
  | "waf_log";

export interface GeneralBlacklistRecord {
  ip: string;
  source?: GeneralBlacklistSource | string;
  comment?: string;
  created_at?: string;
  updated_at?: string;
}

export interface GeneralBlacklistList {
  total: number;
  items: GeneralBlacklistRecord[];
}

export interface GeneralBlacklistMutationResult {
  added: number;
  updated: number;
  removed: number;
  total: number;
  items: GeneralBlacklistRecord[];
}

export interface GeneralBlacklistStatus {
  records: Record<string, GeneralBlacklistRecord>;
}

export type WAFMode = "off" | "detection" | "blocking";

export interface WAFConfig {
  enabled: boolean;
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

export interface IptablesInitRequest {
  chain_name?: string;
  parent_chain?: string[];
  exempt_ports?: string[];
}

export interface IpRequest {
  ip: string;
}

export interface TcpRedirectRequest {
  listen_port: number;
  target_port: number;
}

export interface TcpPortRuleRequest {
  ip: string;
  port: number;
}

export interface SSHFirewallSyncRequest {
  chain_name?: string;
  parent_chain?: string[];
  ports: number[];
  allowed_cidrs: string[];
  blocked_ips?: string[];
  include_local_cidrs?: boolean;
}

export interface SSHFirewallClearRequest {
  chain_name?: string;
  parent_chain?: string[];
}
