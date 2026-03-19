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
  auth_cache_expire?: number;
  public_auth_base_url?: string;
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
}

export interface IptablesInitRequest {
  chain_name?: string;
  parent_chain?: string[];
  exempt_ports?: string[];
}

export interface IpRequest {
  ip: string;
}

export class GoBackendService {
  private baseUrl: string;
  private requestTimeoutMs: number;
  private trafficApiUnavailable = false;
  private trafficApiUnavailableLogged = false;
  private lastTrafficStats: TrafficStats = {
    total_in: 0,
    total_out: 0,
    active_conns: 0,
    error_5xx: 0,
  };

  constructor(
    baseUrl: string = `http://localhost:${process.env.GO_BACKEND_PORT || 7996}` ||
      "http://localhost:7996",
  ) {
    this.baseUrl = baseUrl;
    this.requestTimeoutMs = this.parseTimeout(
      process.env.GO_BACKEND_TIMEOUT_MS,
      5000,
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
    return this.request<HostRule[]>("/api/host-rules", "POST", rules);
  }

  async flushHostRules(): Promise<GoResponse> {
    return this.request("/api/host-rules", "DELETE");
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

  async allowAll(): Promise<GoResponse> {
    return this.request("/api/iptables/allow-all", "POST");
  }

  async blockAll(): Promise<GoResponse> {
    return this.request("/api/iptables/block-all", "POST");
  }
}

export const goBackend = new GoBackendService();
