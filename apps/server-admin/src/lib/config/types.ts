import type { AcmeCertificateAuthority } from "../acme-certificate-authority";
import type { AutoHttpsConfig } from "../auto-https-redirect";
import type { ReverseProxySubmode } from "../reverse-proxy-submode";
import type { SSHSecurityConfig } from "../ssh-security/types";
import type { TerminalFeatureConfig } from "../terminal-shared";
import type { TOTPSubdomainAccess } from "../totp-subdomain-access";
import type { TOTPAccessScope } from "../totp-access-scopes";
import type { LocaleConfig } from "../../../../../packages/i18n/src";
import type { AppearanceConfig } from "../../../../../packages/admin-shared/src/utils/appearance";

export interface ProxyMapping {
  path: string;
  target: string;
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
}

export type RunType = 0 | 1 | 3;

export interface WelcomeGuideStatus {
  completed: boolean;
  completed_at: string | null;
}

export type HostAccessMode = "login_first" | "strict_whitelist";
export type HostServiceRole = "app" | "auth";
export type StreamMappingProtocol = "tcp" | "udp";

export interface HostMappingBasicAuth {
  enabled: boolean;
  username: string;
  password: string;
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
  use_auth: boolean;
  access_mode: HostAccessMode;
  suppress_toolbar: boolean;
  preserve_host: boolean;
  basic_auth: HostMappingBasicAuth;
  locations: HostLocation[];
  service_role: HostServiceRole;
  title: string;
  title_override: string;
  favicon: string;
}

export interface StreamMapping {
  protocol: StreamMappingProtocol;
  listen_port: number;
  target: string;
  use_auth: boolean;
}

export type PasskeyRpMode = "auth_host" | "parent_domain";

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
  cert: string;
  key: string;
  active_cert_id?: string;
  deployment_mode?: SSLDeploymentMode;
  certificates?: SSLManagedCertificate[];
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

export interface SSLManagedCertificate {
  id: string;
  label: string;
  source: SSLCertificateSource;
  primary_domain?: string;
  source_ref_id?: string;
  cert: string;
  key: string;
  created_at: string;
  updated_at: string;
}

export interface SSLCertificateSummary {
  id: string;
  label: string;
  source: SSLCertificateSource;
  primary_domain?: string;
  source_ref_id?: string;
  created_at: string;
  updated_at: string;
  certInfo?: SSLCertInfo;
  is_active: boolean;
}

export interface SSLStatus {
  enabled: boolean;
  activeCertId?: string;
  deploymentMode: SSLDeploymentMode;
  certInfo?: SSLCertInfo;
  certificates: SSLCertificateSummary[];
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

export interface GatewayLoggingSettings {
  enabled: boolean;
  max_days: number;
}

export type WAFMode = "off" | "detection" | "blocking";

export interface WAFConfig {
  enabled: boolean;
  system_rules_auto_update_enabled: boolean;
  common_location_exempt_enabled: boolean;
  mode: WAFMode;
  active_bundle_id: string;
  rules_dir: string;
  paranoia_level: 1 | 2 | 3 | 4;
  executing_paranoia_level: 1 | 2 | 3 | 4;
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

export interface ReverseProxyThrottleConfig {
  enabled: boolean;
  requests_per_second: number;
  burst: number;
  block_seconds: number;
}

export interface EventSystemSimpleRuleConfig {
  enabled: boolean;
}

export interface EventSystemResourceAlertRuleConfig {
  enabled: boolean;
  threshold_percent: number;
  recover_percent: number;
  sample_interval_seconds: number;
  sustain_seconds: number;
}

export interface EventSystemConfig {
  enabled: boolean;
  retention_days: number;
  rules: {
    login_failure: EventSystemSimpleRuleConfig;
    ip_drift: EventSystemSimpleRuleConfig;
    scanner_blocked: EventSystemSimpleRuleConfig;
    ddns_update: EventSystemSimpleRuleConfig;
    gateway_throttle_block: EventSystemSimpleRuleConfig;
    waf_blocked: EventSystemSimpleRuleConfig;
    app_update_available: EventSystemSimpleRuleConfig;
    frp_tunnel: EventSystemSimpleRuleConfig;
    cloudflared_tunnel: EventSystemSimpleRuleConfig;
    ssh_login_success: EventSystemSimpleRuleConfig;
    ssh_login_failure: EventSystemSimpleRuleConfig;
    ssh_ip_blocked: EventSystemSimpleRuleConfig;
    cpu_alert: EventSystemResourceAlertRuleConfig;
    memory_alert: EventSystemResourceAlertRuleConfig;
  };
}

export interface GatewayVisibilitySelection {
  province: string;
  city: string | null;
  label: string;
  value: string;
  query_city: string | null;
  is_province_wide: boolean;
  is_municipality: boolean;
}

export interface GatewayVisibilityConfig {
  enabled: boolean;
  selections: GatewayVisibilitySelection[];
  custom_cidrs: string[];
}

export interface GatewayVisibilityRuntimeState {
  enabled: boolean;
  cidrs: string[];
  updated_at: string | null;
}

export interface GatewayProxyHeadersConfig {
  disabled_hosts: string[];
}

export interface GatewayProxyHeadersRuntimeState {
  enabled: boolean;
  omit_targets: string[];
  updated_at: string | null;
}

export interface GatewayHostResponseConfig {
  disabled_hosts: string[];
}

export interface GatewayHostResponseRuntimeState {
  enabled: boolean;
  omit_targets: string[];
  updated_at: string | null;
}

export type GatewayPortalDisplayStyle = "domain" | "title";

export interface GatewayPortalConfig {
  enabled: boolean;
  display_style: GatewayPortalDisplayStyle;
  show_app_icon: boolean;
}

export interface ReverseProxyTrustedIPRuntimeItem {
  ip: string;
  sources: string[];
}

export interface ReverseProxyTrustedIPRuntimeState {
  enabled: boolean;
  items: ReverseProxyTrustedIPRuntimeItem[];
  cidrs: string[];
  updated_at: string | null;
}

export interface ProtocolMappingFeatureConfig {
  enabled: boolean;
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

export type CaptchaProvider = "pow" | "turnstile";

export type CaptchaWidgetMode = "normal";

export type TurnstileCaptchaConfig = {
  site_key: string;
  secret_key: string;
};

export type CaptchaSettings = {
  provider: CaptchaProvider;
  widget_mode: CaptchaWidgetMode;
  pow: Record<string, never>;
  turnstile: TurnstileCaptchaConfig;
};

export type IpLocationApiMode = "online" | "custom";

export type IpLocationApiConfig = {
  ip_lookup_mode: IpLocationApiMode;
  ip_lookup_url: string;
  cidr_mode: IpLocationApiMode;
  cidr_url: string;
};

export type AcmeJobStatus =
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "stopped";
export type AcmeJobMethod = "dns" | "http" | "https";
export type AcmeJobTrigger = "manual_request" | "auto_renew";
export type AcmeApplicationLatestJobStatus = AcmeJobStatus | "idle";
export type AcmeJob = {
  id: string;
  applicationId?: string;
  domains: string[];
  method: AcmeJobMethod;
  provider: string | null;
  trigger?: AcmeJobTrigger;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
  status: AcmeJobStatus;
  progress: number;
  message?: string;
};

export type AcmeApplication = {
  id: string;
  name?: string;
  domains: string[];
  primaryDomain: string;
  dnsType: string;
  credentials: Record<string, string>;
  renewEnabled: boolean;
  createdAt: string;
  updatedAt: string;
  latestJobId?: string;
  latestJobStatus?: AcmeApplicationLatestJobStatus;
  latestJobTrigger?: AcmeJobTrigger;
  latestJobAt?: string;
  lastError?: string;
};

export type AcmeIssuedCertificate = {
  applicationId: string;
  primaryDomain: string;
  cert: string;
  key: string;
  certInfo: SSLCertInfo;
  createdAt: string;
  updatedAt: string;
  libraryCertificateId?: string;
  libraryLinkedAt?: string;
};

export type AcmeRuntimeLock = {
  locked: boolean;
  lockId?: string;
  jobId?: string;
  applicationId?: string;
  reason?: AcmeJobTrigger;
  startedAt?: string;
  heartbeatAt?: string;
  expiresAt?: string;
};

export type AcmeApplicationSaveResult = {
  application: AcmeApplication;
  certificateInvalidated: boolean;
  deletedIssuedCertificate: AcmeIssuedCertificate | null;
  removedLibraryCertificates: SSLManagedCertificate[];
  removedActiveLibraryCertificate: boolean;
  removedDomains: string[];
};

export type AcmeApplicationDeleteResult = {
  application: AcmeApplication;
  deletedIssuedCertificate: AcmeIssuedCertificate | null;
  removedLibraryCertificates: SSLManagedCertificate[];
  removedActiveLibraryCertificate: boolean;
  removedDomains: string[];
};

export type AcmeSettings = {
  domains: string[];
  dnsType: string;
  credentials: Record<string, string>;
  updatedAt: string;
};

export type AcmeClientSettings = {
  certificateAuthority: AcmeCertificateAuthority;
  updatedAt: string;
};

export type LoginSession = {
  totpId: string;
  method: "TOTP" | "PASSKEY" | "OIDC";
  credentialId: string;
  credentialName: string;
  linkedTotpName?: string;
  grantType?: "browser_session" | "login_ip_grant";
  postLoginIpGrantMode?: PostLoginIpGrantMode | null;
  postLoginIpGrantRecordId?: string | null;
  comment?: string;
  ip: string;
  userAgent: string;
  loginTime: string;
  expiresAt?: string;
  ipLocation?: string;
};

export interface AppConfig {
  run_type: RunType;
  reverse_proxy_submode: ReverseProxySubmode;
  auto_manage_firewall: boolean;
  whitelist_ips: string[];
  proxy_mappings: ProxyMapping[];
  host_mappings: HostMapping[];
  stream_mappings: StreamMapping[];
  subdomain_mode: SubdomainModeConfig;
  ssl: SSLConfig;
  default_route: string;
  default_tunnel?: "frp" | "cloudflared";
  fnos_share_bypass?: FnosShareBypassConfig;
  fnos_port_icon_hijack?: FnosPortIconHijackConfig;
  gateway_logging?: GatewayLoggingSettings;
  waf?: WAFConfig;
  reverse_proxy_throttle?: ReverseProxyThrottleConfig;
  gateway_visibility?: GatewayVisibilityConfig;
  gateway_proxy_headers?: GatewayProxyHeadersConfig;
  gateway_host_response?: GatewayHostResponseConfig;
  gateway_portal?: GatewayPortalConfig;
  appearance?: AppearanceConfig;
  dashboard_display?: DashboardDisplayConfig;
  auto_https?: AutoHttpsConfig;
  smart_connect?: SmartConnectConfig;
  scan_discovery?: ScanDiscoveryConfig;
  auth_credential_settings?: AuthCredentialSettings;
  event_system?: EventSystemConfig;
  terminal_feature?: TerminalFeatureConfig;
  ssh_security?: SSHSecurityConfig;
  locale?: LocaleConfig;
}

export interface RunModePromptPreferences {
  directToReverseProxy: boolean;
  reverseProxyToDirect: boolean;
  switchToSubdomain: boolean;
  subdomainToReverseProxy: boolean;
}

export type PostLoginIpGrantMode = "follow_session" | "disabled" | "custom";

export interface AuthCredentialSettings {
  session_ttl_seconds: number;
  remember_me_ttl_seconds: number;
  post_login_ip_grant_mode: PostLoginIpGrantMode;
  post_login_ip_grant_ttl_seconds: number | null;
  session_ip_mobility_enabled: boolean;
  session_ip_mobility_window_seconds: number;
  passkey_bind_prompt_enabled: boolean;
}

export type TOTPCredential = {
  id: string;
  secret: string;
  comment: string;
  createdAt: string;
  access_scopes: TOTPAccessScope[];
  subdomain_access: TOTPSubdomainAccess;
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
