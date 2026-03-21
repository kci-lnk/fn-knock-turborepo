export interface ProxyMapping {
  path: string;
  target: string;
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
}

export type RunType = 0 | 1 | 3;

export type HostAccessMode = "login_first" | "strict_whitelist";
export type HostServiceRole = "app" | "auth";

export interface HostMapping {
  host: string;
  target: string;
  use_auth: boolean;
  access_mode: HostAccessMode;
  suppress_toolbar: boolean;
  preserve_host: boolean;
  service_role: HostServiceRole;
}

export type PasskeyRpMode = "auth_host" | "parent_domain";

export interface SubdomainModeConfig {
  root_domain: string;
  auth_host: string;
  auth_target: string;
  cookie_domain: string;
  public_auth_base_url: string;
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

export interface GatewayLoggingConfig {
  enabled: boolean;
  max_days: number;
  logs_dir: string;
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
  x_forwarded_for?: string;
  x_real_ip?: string;
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
  page: number;
  limit: number;
  total: number;
  items: GatewayLogEntry[];
}

export interface GatewayLogDeletePayload {
  date: string;
  logs_dir: string;
  deleted: boolean;
  available_dates: string[];
}

export interface TerminalFeatureConfig {
  enabled: boolean;
  default_shell: string;
  default_cwd: string;
  max_sessions: number;
  idle_timeout_seconds: number;
  resume_backend: "tmux";
  allow_mobile_toolbar: boolean;
  dangerously_run_as_current_user: boolean;
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
  httpPollingAvailable: boolean;
  runningAsRoot: boolean;
  blockedReason: string;
}

export interface AppConfig {
  run_type: RunType;
  whitelist_ips: string[];
  default_route: string;
  proxy_mappings: ProxyMapping[];
  host_mappings: HostMapping[];
  subdomain_mode: SubdomainModeConfig;
  default_tunnel?: "frp" | "cloudflared";
  fnos_share_bypass?: FnosShareBypassConfig;
  gateway_logging?: GatewayLoggingConfig;
  terminal_feature?: TerminalFeatureConfig;
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

export type LoginSession = {
  totpId: string;
  method: "TOTP" | "PASSKEY";
  credentialId: string;
  credentialName: string;
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
  lastDriftSource: "proxy-session" | "fnos-token" | null;
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
      source: "proxy-session" | "fnos-token";
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

export type SessionMobilityDetails = {
  summary: SessionMobilitySummary;
  events: SessionMobilityEvent[];
};

export type SessionRecord = LoginSession & {
  id: string;
  mobility?: SessionMobilitySummary;
};

export type ProxyProtocolForce = {
  proxy_protocol_force: boolean;
};

export type TrafficStats = {
  total_in: number;
  total_out: number;
  active_conns: number;
  error_5xx: number;
  timestamp: number;
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
  };
  series: {
    failedLogins: Array<[number, number]>;
    blockedScanners: Array<[number, number]>;
  };
};
