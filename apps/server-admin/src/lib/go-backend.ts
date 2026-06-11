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
  basic_auth?: {
    enabled: boolean;
    username: string;
    password: string;
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

export class GoBackendService {
  private baseUrl: string;
  private requestTimeoutMs: number;
  private sshFirewallTimeoutMs: number;
  private trafficApiUnavailable = false;
  private trafficApiUnavailableLogged = false;
  private trafficActiveIPsApiUnavailable = false;
  private trafficActiveIPsApiUnavailableLogged = false;
  private lastTrafficStats: TrafficStats = {
    total_in: 0,
    total_out: 0,
    active_conns: 0,
    error_5xx: 0,
    by_host: [],
  };

  constructor(
    baseUrl: string = process.env.GO_BACKEND_BASE_URL?.trim() ||
      `http://localhost:${process.env.GO_BACKEND_PORT || 7996}`,
  ) {
    this.baseUrl = baseUrl;
    this.requestTimeoutMs = this.parseTimeout(
      process.env.GO_BACKEND_TIMEOUT_MS,
      5000,
    );
    this.sshFirewallTimeoutMs = this.parseTimeout(
      process.env.GO_BACKEND_SSH_FIREWALL_TIMEOUT_MS,
      Math.max(this.requestTimeoutMs, 30000),
    );
  }

  private parseTimeout(raw: string | undefined, fallback: number): number {
    const value = Number.parseInt(String(raw ?? ""), 10);
    if (!Number.isFinite(value) || value <= 0) return fallback;
    return value;
  }

  private async request<T = unknown>(
    path: string,
    method: string = "GET",
    body?: unknown,
    timeoutMs: number = this.requestTimeoutMs,
    options?: { suppressStatusLog?: number[] },
  ): Promise<GoResponse<T>> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const res = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: { "Content-Type": "application/json" },
        body: body !== undefined ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        const suppressed =
          options?.suppressStatusLog?.includes(res.status) ?? false;
        if (!suppressed) {
          console.error(
            `[GoBackend] ${method} ${path} failed: ${res.status} ${res.statusText}`,
            text,
          );
        }
        return {
          success: false,
          code: res.status,
          message: `${res.status} ${res.statusText}`,
        };
      }

      try {
        return (await res.json()) as GoResponse<T>;
      } catch (e: any) {
        console.error(
          `[GoBackend] ${method} ${path} invalid JSON response:`,
          e,
        );
        return {
          success: false,
          code: 502,
          message: "Invalid JSON from go-backend",
        };
      }
    } catch (e: any) {
      if (e?.name === "AbortError") {
        console.error(
          `[GoBackend] ${method} ${path} timeout after ${timeoutMs}ms`,
        );
        return {
          success: false,
          code: 504,
          message: `Go backend timeout (${timeoutMs}ms)`,
        };
      }
      console.error(`[GoBackend] ${method} ${path} error:`, e);
      return { success: false, code: 502, message: e?.message ?? String(e) };
    } finally {
      clearTimeout(timer);
    }
  }

  async getAuthConfig(): Promise<GoResponse<AuthConfig>> {
    return this.request<AuthConfig>("/api/auth");
  }

  async setAuthConfig(config: AuthConfig): Promise<GoResponse> {
    return this.request("/api/auth", "POST", config);
  }

  async getDefaultRoute(): Promise<GoResponse<string>> {
    return this.request<string>("/api/config/default-route");
  }

  async setDefaultRoute(route: string): Promise<GoResponse> {
    return this.request("/api/config/default-route", "POST", {
      default_route: route,
    });
  }

  async getProxyProtocolForce(): Promise<
    GoResponse<ProxyProtocolForceResponse>
  > {
    return this.request<ProxyProtocolForceResponse>(
      "/api/config/proxy-protocol",
    );
  }

  async setProxyProtocolForce(
    proxy_protocol_force: boolean,
  ): Promise<GoResponse<ProxyProtocolForceResponse>> {
    return this.request<ProxyProtocolForceResponse>(
      "/api/config/proxy-protocol",
      "POST",
      { proxy_protocol_force } satisfies ProxyProtocolForceRequest,
    );
  }

  async getReverseProxyThrottle(): Promise<
    GoResponse<ReverseProxyThrottleConfig>
  > {
    return this.request<ReverseProxyThrottleConfig>(
      "/api/config/reverse-proxy-throttle",
    );
  }

  async setReverseProxyThrottle(
    config: ReverseProxyThrottleConfig,
  ): Promise<GoResponse<ReverseProxyThrottleConfig>> {
    return this.request<ReverseProxyThrottleConfig>(
      "/api/config/reverse-proxy-throttle",
      "POST",
      config,
    );
  }

  async getGatewayVisibility(): Promise<GoResponse<GatewayVisibilityConfig>> {
    return this.request<GatewayVisibilityConfig>("/api/config/visibility");
  }

  async setGatewayVisibility(
    config: GatewayVisibilityConfig,
  ): Promise<GoResponse<GatewayVisibilityConfig>> {
    const payload = {
      enabled: config.enabled,
      cidrs: config.cidrs,
      ...(config.updated_at ? { updated_at: config.updated_at } : {}),
    };
    return this.request<GatewayVisibilityConfig>(
      "/api/config/visibility",
      "POST",
      payload,
    );
  }

  async getForwardedHeadersConfig(): Promise<
    GoResponse<ForwardedHeadersConfig>
  > {
    return this.request<ForwardedHeadersConfig>(
      "/api/config/forwarded-headers",
    );
  }

  async setForwardedHeadersConfig(
    config: ForwardedHeadersConfig,
  ): Promise<GoResponse<ForwardedHeadersConfig>> {
    const payload = {
      enabled: config.enabled,
      omit_targets: config.omit_targets,
      ...(config.updated_at ? { updated_at: config.updated_at } : {}),
    };

    return this.request<ForwardedHeadersConfig>(
      "/api/config/forwarded-headers",
      "POST",
      payload,
    );
  }

  async getPreserveHostConfig(): Promise<GoResponse<PreserveHostConfig>> {
    return this.request<PreserveHostConfig>("/api/config/preserve-host");
  }

  async setPreserveHostConfig(
    config: PreserveHostConfig,
  ): Promise<GoResponse<PreserveHostConfig>> {
    const payload = {
      enabled: config.enabled,
      omit_targets: config.omit_targets,
      ...(config.updated_at ? { updated_at: config.updated_at } : {}),
    };

    return this.request<PreserveHostConfig>(
      "/api/config/preserve-host",
      "POST",
      payload,
    );
  }

  async getFnosPortIconHijackConfig(): Promise<
    GoResponse<FnosPortIconHijackConfig>
  > {
    return this.request<FnosPortIconHijackConfig>(
      "/api/config/fnos-port-icon-hijack",
    );
  }

  async setFnosPortIconHijackConfig(
    config: FnosPortIconHijackConfig,
  ): Promise<GoResponse<FnosPortIconHijackConfig>> {
    const payload = {
      enabled: config.enabled,
      ...(config.updated_at ? { updated_at: config.updated_at } : {}),
    };

    return this.request<FnosPortIconHijackConfig>(
      "/api/config/fnos-port-icon-hijack",
      "POST",
      payload,
    );
  }

  async getReverseProxyThrottleExemptIPs(): Promise<
    GoResponse<ReverseProxyThrottleExemptIPsRuntime>
  > {
    return this.request<ReverseProxyThrottleExemptIPsRuntime>(
      "/api/runtime/reverse-proxy-throttle-exempt-ips",
      "GET",
      undefined,
      this.requestTimeoutMs,
      { suppressStatusLog: [404] },
    );
  }

  async setReverseProxyThrottleExemptIPs(
    config: ReverseProxyThrottleExemptIPsRuntime,
  ): Promise<GoResponse<ReverseProxyThrottleExemptIPsRuntime>> {
    return this.request<ReverseProxyThrottleExemptIPsRuntime>(
      "/api/runtime/reverse-proxy-throttle-exempt-ips",
      "POST",
      config,
      this.requestTimeoutMs,
      { suppressStatusLog: [404] },
    );
  }

  async setCommonLocationExemptions(
    config: CommonLocationExemptionsRuntime,
  ): Promise<GoResponse<CommonLocationExemptionsRuntime>> {
    return this.request<CommonLocationExemptionsRuntime>(
      "/api/runtime/common-location-exemptions",
      "POST",
      config,
      this.requestTimeoutMs,
      { suppressStatusLog: [404] },
    );
  }

  async getServerInfo(): Promise<GoResponse<ServerInfo>> {
    return this.request<ServerInfo>("/api/info");
  }

  async getTrafficStats(): Promise<GoResponse<TrafficStats>> {
    if (this.trafficApiUnavailable) {
      return {
        success: true,
        code: 200,
        message: "Traffic API unavailable; fallback snapshot",
        data: { ...this.lastTrafficStats },
      };
    }

    const resp = await this.request<TrafficStats>(
      "/api/traffic",
      "GET",
      undefined,
      this.requestTimeoutMs,
      { suppressStatusLog: [404] },
    );

    if (resp.success && resp.data) {
      this.lastTrafficStats = { ...resp.data };
      return resp;
    }

    if (resp.code === 404) {
      this.trafficApiUnavailable = true;
      if (!this.trafficApiUnavailableLogged) {
        this.trafficApiUnavailableLogged = true;
        console.warn(
          `[GoBackend] ${this.baseUrl}/api/traffic is not supported by current gateway; using fallback traffic snapshot.`,
        );
      }
      return {
        success: true,
        code: 200,
        message: "Traffic API unavailable; fallback snapshot",
        data: { ...this.lastTrafficStats },
      };
    }

    return resp;
  }

  async getHostActiveIPs(
    host: string,
  ): Promise<GoResponse<HostActiveIPsStats>> {
    const fallback: HostActiveIPsStats = {
      host,
      window_seconds: 120,
      items: [],
    };

    if (this.trafficApiUnavailable || this.trafficActiveIPsApiUnavailable) {
      return {
        success: true,
        code: 200,
        message: "Traffic active IPs API unavailable; fallback snapshot",
        data: fallback,
      };
    }

    const resp = await this.request<HostActiveIPsStats>(
      `/api/traffic/active-ips?host=${encodeURIComponent(host)}`,
      "GET",
      undefined,
      this.requestTimeoutMs,
      { suppressStatusLog: [404] },
    );

    if (resp.success && resp.data) {
      return resp;
    }

    if (resp.code === 404) {
      this.trafficActiveIPsApiUnavailable = true;
      if (!this.trafficActiveIPsApiUnavailableLogged) {
        this.trafficActiveIPsApiUnavailableLogged = true;
        console.warn(
          `[GoBackend] ${this.baseUrl}/api/traffic/active-ips is not supported by current gateway; using empty active IP snapshot.`,
        );
      }
      return {
        success: true,
        code: 200,
        message: "Traffic active IPs API unavailable; fallback snapshot",
        data: fallback,
      };
    }

    return resp;
  }

  async getGatewayLoggingConfig(): Promise<GoResponse<GatewayLoggingConfig>> {
    return this.request<GatewayLoggingConfig>("/api/logging");
  }

  async setGatewayLoggingConfig(
    config: Pick<GatewayLoggingConfig, "enabled" | "max_days">,
  ): Promise<GoResponse<GatewayLoggingConfig>> {
    return this.request<GatewayLoggingConfig>("/api/logging", "POST", config);
  }

  async getGatewayLoggingDirectory(): Promise<
    GoResponse<GatewayLoggingDirectory>
  > {
    return this.request<GatewayLoggingDirectory>("/api/logging/directory");
  }

  async getGatewayLogDates(): Promise<GoResponse<GatewayLogDates>> {
    return this.request<GatewayLogDates>("/api/logging/dates");
  }

  async getGatewayLogEntries(params: {
    date?: string;
    pagination?: string;
    page?: string | number;
    limit?: string | number;
    cursor?: string;
    search?: string;
    status?: string;
    logged_in?: string;
    waf_status?: string;
  }): Promise<GoResponse<GatewayLogEntriesResponse>> {
    const searchParams = new URLSearchParams();
    if (params.date) searchParams.set("date", params.date);
    if (params.pagination) searchParams.set("pagination", params.pagination);
    if (params.page !== undefined)
      searchParams.set("page", String(params.page));
    if (params.limit !== undefined) {
      searchParams.set("limit", String(params.limit));
    }
    if (params.cursor) searchParams.set("cursor", params.cursor);
    if (params.search) searchParams.set("search", params.search);
    if (params.status) searchParams.set("status", params.status);
    if (params.logged_in) searchParams.set("logged_in", params.logged_in);
    if (params.waf_status) searchParams.set("waf_status", params.waf_status);
    const query = searchParams.toString();
    return this.request<GatewayLogEntriesResponse>(
      `/api/logging/entries${query ? `?${query}` : ""}`,
    );
  }

  async deleteGatewayLogEntries(
    date: string,
  ): Promise<GoResponse<GatewayLogDeleteResponse>> {
    return this.request<GatewayLogDeleteResponse>(
      "/api/logging/entries",
      "DELETE",
      { date },
    );
  }

  async getWAFStatus(): Promise<GoResponse<WAFStatus>> {
    return this.request<WAFStatus>("/api/waf/status");
  }

  async setWAFConfig(config: WAFConfig): Promise<GoResponse<WAFStatus>> {
    return this.request<WAFStatus>("/api/waf/config", "POST", config);
  }

  async reloadWAFRules(config: WAFConfig): Promise<GoResponse<WAFStatus>> {
    return this.request<WAFStatus>("/api/waf/reload", "POST", { config });
  }

  async drainWAFEvents(limit: number): Promise<GoResponse<WAFDrainResult>> {
    return this.request<WAFDrainResult>("/api/waf/events/drain", "POST", {
      limit,
    });
  }

  async getRules(): Promise<GoResponse<Rule[]>> {
    return this.request<Rule[]>("/api/rules");
  }

  async setRules(rules: Rule[]): Promise<GoResponse<Rule[]>> {
    return this.request<Rule[]>("/api/rules", "POST", rules);
  }

  async flushRules(): Promise<GoResponse> {
    return this.request("/api/rules", "DELETE");
  }

  async getHostRules(): Promise<GoResponse<HostRule[]>> {
    return this.request<HostRule[]>("/api/host-rules");
  }

  async setHostRules(rules: HostRule[]): Promise<GoResponse<HostRule[]>> {
    return this.request<HostRule[]>(
      "/api/host-rules",
      "POST",
      rules.map((rule) => ({
        host: rule.host,
        target: rule.target,
        use_auth: rule.use_auth,
        access_mode: rule.access_mode,
        suppress_toolbar: rule.suppress_toolbar,
        preserve_host: rule.preserve_host,
        basic_auth: rule.basic_auth,
      })),
    );
  }

  async flushHostRules(): Promise<GoResponse> {
    return this.request("/api/host-rules", "DELETE");
  }

  async getStreamRules(): Promise<GoResponse<StreamRule[]>> {
    return this.request<StreamRule[]>("/api/stream-rules");
  }

  async setStreamRules(rules: StreamRule[]): Promise<GoResponse<StreamRule[]>> {
    return this.request<StreamRule[]>("/api/stream-rules", "POST", rules);
  }

  async flushStreamRules(): Promise<GoResponse> {
    return this.request("/api/stream-rules", "DELETE");
  }

  async getSSLStatus(): Promise<GoResponse<SSLInfo>> {
    return this.request<SSLInfo>("/api/ssl");
  }

  async setSSLDeployment(
    deployment: SSLDeploymentRequest,
  ): Promise<GoResponse> {
    return this.request("/api/ssl", "POST", deployment);
  }

  async setSSL(cert: string, key: string): Promise<GoResponse> {
    return this.setSSLDeployment({ cert, key } satisfies SSLRequest);
  }

  async clearSSL(): Promise<GoResponse> {
    return this.request("/api/ssl", "DELETE");
  }

  async initIptables(opts?: IptablesInitRequest): Promise<GoResponse> {
    return this.request("/api/iptables/init", "POST", opts);
  }

  async listIptables(): Promise<GoResponse<string[]>> {
    return this.request<string[]>("/api/iptables/list");
  }

  async flushIptables(): Promise<GoResponse> {
    return this.request("/api/iptables/flush", "POST");
  }

  async cleanIptables(): Promise<GoResponse> {
    return this.request("/api/iptables/clean", "POST");
  }

  async ensureTCPRedirect(
    listenPort: number,
    targetPort: number,
  ): Promise<GoResponse> {
    return this.request("/api/iptables/tcp-redirect", "POST", {
      listen_port: listenPort,
      target_port: targetPort,
    } satisfies TcpRedirectRequest);
  }

  async clearTCPRedirect(
    listenPort: number,
    targetPort: number,
  ): Promise<GoResponse> {
    return this.request("/api/iptables/tcp-redirect", "DELETE", {
      listen_port: listenPort,
      target_port: targetPort,
    } satisfies TcpRedirectRequest);
  }

  async allowIP(ip: string): Promise<GoResponse> {
    return this.request("/api/iptables/allow", "POST", {
      ip,
    } satisfies IpRequest);
  }

  async removeIP(ip: string): Promise<GoResponse> {
    return this.request("/api/iptables/remove", "POST", {
      ip,
    } satisfies IpRequest);
  }

  async blockIP(ip: string): Promise<GoResponse> {
    return this.request("/api/iptables/block", "POST", {
      ip,
    } satisfies IpRequest);
  }

  async blockTCPPortForIP(ip: string, port: number): Promise<GoResponse> {
    return this.request("/api/iptables/tcp-port/block", "POST", {
      ip,
      port,
    } satisfies TcpPortRuleRequest);
  }

  async removeTCPPortRule(ip: string, port: number): Promise<GoResponse> {
    return this.request("/api/iptables/tcp-port/remove", "POST", {
      ip,
      port,
    } satisfies TcpPortRuleRequest);
  }

  async syncSSHFirewall(payload: SSHFirewallSyncRequest): Promise<GoResponse> {
    return this.request(
      "/api/iptables/ssh/sync",
      "POST",
      payload,
      this.sshFirewallTimeoutMs,
    );
  }

  async clearSSHFirewall(
    payload: SSHFirewallClearRequest = {},
  ): Promise<GoResponse> {
    return this.request(
      "/api/iptables/ssh/clear",
      "POST",
      payload,
      this.sshFirewallTimeoutMs,
    );
  }

  async allowAll(): Promise<GoResponse> {
    return this.request("/api/iptables/allow-all", "POST");
  }

  async blockAll(): Promise<GoResponse> {
    return this.request("/api/iptables/block-all", "POST");
  }
}

export const goBackend = new GoBackendService();
