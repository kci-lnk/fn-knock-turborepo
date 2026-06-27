import type {
  AuthConfig,
  CommonLocationExemptionsRuntime,
  CrawlerBlockerConfig,
  FnosPortIconHijackConfig,
  ForwardedHeadersConfig,
  GatewayLogDates,
  GatewayLogDeleteResponse,
  GatewayLogEntriesResponse,
  GatewayLoggingConfig,
  GatewayLoggingDirectory,
  GatewayPortalConfig,
  GatewayVisibilityConfig,
  GeneralBlacklistList,
  GeneralBlacklistMutationResult,
  GeneralBlacklistSource,
  GeneralBlacklistStatus,
  GoResponse,
  HostActiveIPsStats,
  HostRule,
  IpRequest,
  IptablesInitRequest,
  LocaleConfig,
  PreserveHostConfig,
  ProxyProtocolForceRequest,
  ProxyProtocolForceResponse,
  ReverseProxyThrottleConfig,
  ReverseProxyThrottleExemptIPsRuntime,
  Rule,
  ServerInfo,
  SSHFirewallClearRequest,
  SSHFirewallSyncRequest,
  SSLDeploymentRequest,
  SSLInfo,
  SSLRequest,
  StreamRule,
  TcpPortRuleRequest,
  TcpRedirectRequest,
  TrafficStats,
  WAFConfig,
  WAFDrainResult,
  WAFStatus,
} from "./go-backend/types";

export type * from "./go-backend/types";

const resolveHostRuleTitle = (
  rule: Pick<HostRule, "title" | "title_override">,
): string => {
  const override =
    typeof rule.title_override === "string" ? rule.title_override.trim() : "";
  if (override) return override;
  return typeof rule.title === "string" ? rule.title.trim() : "";
};

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

  async getLocaleConfig(): Promise<GoResponse<LocaleConfig>> {
    return this.request<LocaleConfig>("/api/config/locale");
  }

  async setLocaleConfig(
    config: LocaleConfig,
  ): Promise<GoResponse<LocaleConfig>> {
    return this.request<LocaleConfig>("/api/config/locale", "POST", config);
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

  async getCrawlerBlockerConfig(): Promise<GoResponse<CrawlerBlockerConfig>> {
    return this.request<CrawlerBlockerConfig>("/api/config/crawler-blocker");
  }

  async setCrawlerBlockerConfig(
    config: CrawlerBlockerConfig,
  ): Promise<GoResponse<CrawlerBlockerConfig>> {
    const payload = {
      enabled: config.enabled,
      ...(config.updated_at ? { updated_at: config.updated_at } : {}),
    };

    return this.request<CrawlerBlockerConfig>(
      "/api/config/crawler-blocker",
      "POST",
      payload,
    );
  }

  async getGatewayPortalConfig(): Promise<GoResponse<GatewayPortalConfig>> {
    return this.request<GatewayPortalConfig>("/api/config/portal");
  }

  async setGatewayPortalConfig(
    config: GatewayPortalConfig,
  ): Promise<GoResponse<GatewayPortalConfig>> {
    return this.request<GatewayPortalConfig>("/api/config/portal", "POST", {
      enabled: config.enabled !== false,
      display_style: config.display_style === "title" ? "title" : "domain",
      show_app_icon: config.show_app_icon === true,
    } satisfies GatewayPortalConfig);
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
    credential?: string;
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
    if (params.credential) searchParams.set("credential", params.credential);
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

  async getGeneralBlacklist(params: {
    page?: string | number;
    limit?: string | number;
    search?: string;
  }): Promise<GoResponse<GeneralBlacklistList>> {
    const searchParams = new URLSearchParams();
    if (params.page !== undefined)
      searchParams.set("page", String(params.page));
    if (params.limit !== undefined)
      searchParams.set("limit", String(params.limit));
    if (params.search) searchParams.set("search", params.search);
    const query = searchParams.toString();
    return this.request<GeneralBlacklistList>(
      `/api/general-blacklist${query ? `?${query}` : ""}`,
    );
  }

  async addGeneralBlacklist(payload: {
    ips: string[];
    source: GeneralBlacklistSource;
    comment?: string;
  }): Promise<GoResponse<GeneralBlacklistMutationResult>> {
    return this.request<GeneralBlacklistMutationResult>(
      "/api/general-blacklist",
      "POST",
      payload,
    );
  }

  async getGeneralBlacklistStatus(
    ips: string[],
  ): Promise<GoResponse<GeneralBlacklistStatus>> {
    return this.request<GeneralBlacklistStatus>(
      "/api/general-blacklist/status",
      "POST",
      { ips },
    );
  }

  async deleteGeneralBlacklist(
    ips: string[],
  ): Promise<GoResponse<GeneralBlacklistMutationResult>> {
    return this.request<GeneralBlacklistMutationResult>(
      "/api/general-blacklist",
      "DELETE",
      { ips },
    );
  }

  async deleteGeneralBlacklistByIp(
    ip: string,
  ): Promise<GoResponse<GeneralBlacklistMutationResult>> {
    return this.request<GeneralBlacklistMutationResult>(
      `/api/general-blacklist/${encodeURIComponent(ip)}`,
      "DELETE",
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
        title: resolveHostRuleTitle(rule),
        favicon:
          typeof rule.favicon === "string" && rule.favicon.trim()
            ? rule.favicon.trim()
            : null,
        basic_auth: rule.basic_auth,
        locations: (rule.locations ?? []).map((location) => ({
          path: location.path,
          match: location.match,
          action: location.action,
          target: location.target,
          strip_path: location.strip_path,
          rewrite_html: location.rewrite_html,
          response: location.response,
        })),
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
