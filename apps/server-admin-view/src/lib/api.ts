import type { AppConfig, ProxyMapping, SSLConfig, SSLStatus, PasskeyCredential, TOTPCredential, SessionRecord, ProxyProtocolForce, TrafficStats, DashboardStats, ThreatOverview, FnosShareBypassConfig, SessionMobilityDetails, SSLSharedFilesPayload, SharedDataFileEntry } from '../types';
import { createSignedApiClient } from '@frontend-core/api/createSignedApiClient';
import type { CaptchaSettings } from '@frontend-core/captcha/types';

const resolveAppRelativePath = (relativePath: string) => {
    if (typeof window === 'undefined') return relativePath;
    const basePath = window.location.pathname.endsWith('/')
        ? window.location.pathname
        : `${window.location.pathname}/`;
    return new URL(relativePath, `${window.location.origin}${basePath}`).pathname;
};

const runtimeSecretPath = resolveAppRelativePath('./__fn-knock/runtime-hmac-secret');
const adminApiBasePath = resolveAppRelativePath('./api/admin');

const runtimeSecret = typeof window !== 'undefined'
    ? (window as Window & { __FN_KNOCK_HMAC_SECRET__?: string }).__FN_KNOCK_HMAC_SECRET__
    : undefined;
let hmacSecret = import.meta.env.VITE_HMAC_SECRET || runtimeSecret;

const fetchRuntimeHmacSecret = async () => {
    if (hmacSecret) return hmacSecret;
    try {
        const res = await fetch(runtimeSecretPath);
        if (res.ok) {
            const payload = await res.json().catch(() => null) as { data?: { hmacSecret?: string } } | null;
            const next = payload?.data?.hmacSecret?.trim();
            if (next) {
                hmacSecret = next;
                return hmacSecret;
            }
        }
    } catch {
        // ignore and fallback below
    }
    throw new Error("Missing HMAC secret: provide VITE_HMAC_SECRET or serve via backend runtime injection");
};

export const apiClient = createSignedApiClient({
    baseURL: adminApiBasePath,
    hmacSecret,
    getHmacSecret: fetchRuntimeHmacSecret,
});

export const ConfigAPI = {
    async getOnboardingStatus(): Promise<{ completed: boolean }> {
        const res = await apiClient.get('/onboarding/status');
        return res.data.data;
    },
    async completeOnboarding(): Promise<void> {
        await apiClient.post('/onboarding/complete');
    },
    async getConfig(): Promise<AppConfig> {
        const res = await apiClient.get('/config');
        return res.data.data;
    },
    async updateRunType(run_type: 0 | 1): Promise<void> {
        await apiClient.post('/config/run_type', { run_type });
    },
    async updateDefaultTunnel(tunnel: 'frp' | 'cloudflared'): Promise<void> {
        await apiClient.post('/config/default_tunnel', { tunnel });
    },

    async updateProxyMappings(mappings: ProxyMapping[]): Promise<void> {
        await apiClient.post('/config/proxy_mappings', { mappings });
    },
    // SSL
    async getSSLStatus(): Promise<SSLStatus> {
        const res = await apiClient.get('/ssl/status');
        return res.data.data;
    },
    async getSSLSharedFiles(): Promise<SSLSharedFilesPayload> {
        const res = await apiClient.get('/ssl/shared-files');
        return res.data.data;
    },
    async readSSLSharedFile(path: string): Promise<{ file: SharedDataFileEntry; content: string }> {
        const res = await apiClient.get('/ssl/shared-files/content', { params: { path } });
        return res.data.data;
    },
    // CA
    async getCAStatus(): Promise<{ initialized: boolean; info?: any }> {
        const res = await apiClient.get('/ssl/ca/status');
        return res.data.data;
    },
    async initCA(): Promise<void> {
        await apiClient.post('/ssl/ca/init');
    },
    async clearCA(): Promise<void> {
        await apiClient.delete('/ssl/ca');
    },
    async downloadCACert(): Promise<Blob> {
        const res = await apiClient.get('/ssl/ca/cert.pem', { responseType: 'blob' });
        return res.data;
    },
    async getCAHosts(): Promise<string[]> {
        const res = await apiClient.get('/ssl/ca/hosts');
        return res.data.data || [];
    },
    async addCAHost(value: string): Promise<string[]> {
        const res = await apiClient.post('/ssl/ca/hosts', { value });
        return res.data.data || [];
    },
    async removeCAHost(value: string): Promise<string[]> {
        const res = await apiClient.delete('/ssl/ca/hosts', { data: { value } });
        return res.data.data || [];
    },
    async clearCAHosts(): Promise<void> {
        await apiClient.delete('/ssl/ca/hosts', { data: { all: true } });
    },
    async issueAndInstall(): Promise<{ success: boolean; message?: string }> {
        const res = await apiClient.post('/ssl/ca/issue');
        return res.data;
    },
    async downloadServerCert(): Promise<Blob> {
        const res = await apiClient.get('/ssl/ca/server-cert.zip', { responseType: 'blob' });
        return res.data;
    },
    async setSSL(ssl: SSLConfig): Promise<void> {
        await apiClient.post('/ssl', { ssl });
    },
    async deleteSSL(): Promise<void> {
        await apiClient.delete('/ssl');
    },
    async mockAction(action: string, payload?: any): Promise<void> {
        await apiClient.post('/mock-action', { action, payload });
    },
    async updateDefaultRoute(path: string): Promise<void> {
        await apiClient.post('/config/default_route', { path });
    },
    async getProxyProtocolForce(): Promise<ProxyProtocolForce> {
        const res = await apiClient.get('/config/proxy_protocol_force');
        return res.data.data;
    },
    async setProxyProtocolForce(proxy_protocol_force: boolean): Promise<ProxyProtocolForce> {
        const res = await apiClient.post('/config/proxy_protocol_force', { proxy_protocol_force });
        return res.data.data;
    },
    // TOTP
    async getTOTPStatus(): Promise<{ bound: boolean; credentials: TOTPCredential[] }> {
        const res = await apiClient.get('/totp/status');
        return res.data.data;
    },
    async setupTOTP(): Promise<{ secret: string; uri: string }> {
        const res = await apiClient.post('/totp/setup');
        return res.data.data;
    },
    async bindTOTP(secret: string, token: string, comment?: string): Promise<{ success: boolean; message?: string }> {
        const res = await apiClient.post('/totp/bind', { secret, token, comment });
        return res.data;
    },
    async deleteTOTP(id: string): Promise<void> {
        await apiClient.delete(`/totp/${encodeURIComponent(id)}`);
    },
    async updateTOTPComment(id: string, comment: string): Promise<void> {
        await apiClient.patch(`/totp/${encodeURIComponent(id)}/comment`, { comment });
    },
    async getPasskeys(totpId: string): Promise<PasskeyCredential[]> {
        const res = await apiClient.get(`/totp/${encodeURIComponent(totpId)}/passkeys`);
        return res.data.data;
    },
    async deletePasskey(id: string): Promise<void> {
        await apiClient.delete(`/passkeys/${encodeURIComponent(id)}`);
    },
    // 同步路由
    async syncRoutes(): Promise<{ success: boolean; message?: string; data?: { synced_rules: number } }> {
        const res = await apiClient.post('/sync-routes');
        return res.data;
    }
};


export interface WhiteListRecord {
  id: string;
  ip: string;
  expireAt: number | null;
  source: 'manual' | 'auto';
  createdAt: number;
  comment?: string;
  status: 'active' | 'expired' | 'deleted';
  ipLocation?: string;
}

export const WhitelistAPI = {
    async getRecords() {
        const res = await apiClient.get('/whitelist');
        return res.data;
    },
    async addRecord(payload: { ip: string; expireAt: number | null; source: string; comment?: string }) {
        const res = await apiClient.post('/whitelist', payload);
        return res.data;
    },
    async deleteRecord(id: string) {
        const res = await apiClient.delete(`/whitelist/${encodeURIComponent(id)}`);
        return res.data;
    },
    async updateComment(id: string, comment: string) {
        const res = await apiClient.patch(`/whitelist/${encodeURIComponent(id)}/comment`, { comment });
        return res.data;
    }
};

export const AuthLogsAPI = {
    async getLogs(page: number, limit: string, search: string) {
        const res = await apiClient.get('/logs', { params: { page, limit, search } });
        return res.data;
    },
    async deleteLogs(ids: string[]) {
        const res = await apiClient.delete('/logs', { data: { ids } });
        return res.data;
    }
};

export const SecurityAPI = {
    async getOverview(rangeSec: number): Promise<ThreatOverview> {
        const res = await apiClient.get('/security/overview', { params: { rangeSec } });
        return res.data.data;
    }
};

export type ScannerSettings = {
    enabled: boolean;
    windowMinutes: number;
    threshold: number;
    windowSeconds: number;
    blacklistTtlSeconds: number;
};

export type ScannerBlacklistHit = {
    path: string;
    createdAt: number;
};

export type ScannerBlacklistRecord = {
    ip: string;
    ipLocation?: string;
    blockedAt: number;
    windowMinutes: number;
    threshold: number;
    hits: ScannerBlacklistHit[];
};

export type ScannerBlacklistList = {
    items: ScannerBlacklistRecord[];
    total: number;
};

export type AccessEntryInfo = {
    env: "GO_REPROXY_PORT";
    port: string;
    isDefault: boolean;
};

export type RunModePromptPreferences = {
    directToReverseProxy: boolean;
    reverseProxyToDirect: boolean;
};

export type UpdateDownloadStatus = 'idle' | 'downloading' | 'verifying' | 'downloaded' | 'installing' | 'error';

export type UpdateLatestPayload = {
    version: string;
    update_available: boolean;
    force_update: boolean;
    download_url: string;
    sha256: string;
    download_url_arm64: string;
    sha256_arm64: string;
    release_notes: string;
};

export type UpdateStatusPayload = {
    githubUrl: string;
    localVersion: string;
    latest: UpdateLatestPayload | null;
    updateEnabled: boolean;
    hasUpdate: boolean;
    forceUpdate: boolean;
    check: {
        lastCheckedAt: number | null;
        error: string | null;
    };
    download: {
        status: UpdateDownloadStatus;
        percent: number;
        downloadedBytes: number;
        totalBytes: number | null;
        error: string | null;
        targetVersion: string | null;
    };
};

export type UpdateConfirmPayload = {
    version: string;
    completedAt: string;
};

export const SystemAPI = {
    async getAccessEntry(): Promise<AccessEntryInfo> {
        const res = await apiClient.get('/system/access-entry');
        return res.data.data;
    },
    async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
        const res = await apiClient.get('/config/run_mode_prompt_preferences');
        return res.data.data;
    },
    async updateRunModePromptPreferences(
        payload: Partial<RunModePromptPreferences>,
    ): Promise<RunModePromptPreferences> {
        const res = await apiClient.post('/config/run_mode_prompt_preferences', payload);
        return res.data.data;
    },
    async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
        const res = await apiClient.get('/config/fnos_share_bypass');
        return res.data.data;
    },
    async updateFnosShareBypassConfig(
        payload: Partial<FnosShareBypassConfig>,
    ): Promise<FnosShareBypassConfig> {
        const res = await apiClient.post('/config/fnos_share_bypass', payload);
        return res.data.data;
    },
    async getFrpStatus() {
        const res = await apiClient.get('/system/frp/status');
        return res.data;
    },
    async startFrpDownload() {
        const res = await apiClient.post('/system/frp/download');
        return res.data;
    },
    async cancelFrpDownload() {
        const res = await apiClient.post('/system/frp/cancel');
        return res.data;
    },
    async deleteFrp() {
        const res = await apiClient.delete('/system/frp');
        return res.data;
    },
    async getCloudflaredStatus() {
        const res = await apiClient.get('/system/cloudflared/status');
        return res.data;
    },
    async startCloudflaredDownload() {
        const res = await apiClient.post('/system/cloudflared/download');
        return res.data;
    },
    async cancelCloudflaredDownload() {
        const res = await apiClient.post('/system/cloudflared/cancel');
        return res.data;
    },
    async deleteCloudflared() {
        const res = await apiClient.delete('/system/cloudflared');
        return res.data;
    },
    async getTrafficStats(): Promise<TrafficStats> {
        const res = await apiClient.get('/traffic');
        return res.data.data;
    }
};

export const CaptchaAPI = {
    async getSettings(): Promise<CaptchaSettings> {
        const res = await apiClient.get('/config/captcha');
        return res.data.data;
    },
    async updateSettings(payload: CaptchaSettings): Promise<CaptchaSettings> {
        const res = await apiClient.post('/config/captcha', payload);
        return res.data.data;
    },
};

export const UpdateAPI = {
    async getStatus(): Promise<UpdateStatusPayload> {
        const res = await apiClient.get('/update/status');
        return res.data.data;
    },
    async checkNow(): Promise<UpdateStatusPayload> {
        const res = await apiClient.post('/update/check');
        return res.data.data;
    },
    async checkAndDownload(): Promise<{ success: boolean; message?: string; data?: UpdateStatusPayload }> {
        const res = await apiClient.post('/update/check-and-download');
        return res.data;
    },
    async startDownload(): Promise<{ success: boolean; message?: string; data?: UpdateStatusPayload }> {
        const res = await apiClient.post('/update/download');
        return res.data;
    },
    async startInstall(): Promise<{ success: boolean; message?: string }> {
        const res = await apiClient.post('/update/install');
        return res.data;
    },
    async consumeConfirm(): Promise<UpdateConfirmPayload | null> {
        const res = await apiClient.get('/update/confirm');
        return res.data.data || null;
    },
};

export const ScannerAPI = {
    async getSettings(): Promise<ScannerSettings> {
        const res = await apiClient.get('/scanner/settings');
        return res.data.data;
    },
    async saveSettings(payload: { enabled: boolean; windowMinutes: number; threshold: number; blacklistTtlSeconds: number }): Promise<ScannerSettings> {
        const res = await apiClient.post('/scanner/settings', payload);
        return res.data.data;
    },
    async getBlacklist(page: number, limit: string, search: string): Promise<ScannerBlacklistList> {
        const res = await apiClient.get('/scanner/blacklist', { params: { page, limit, search } });
        return res.data.data;
    },
    async getBlacklistDetail(ip: string): Promise<ScannerBlacklistRecord> {
        const res = await apiClient.get(`/scanner/blacklist/${encodeURIComponent(ip)}`);
        return res.data.data;
    },
    async deleteBlacklist(ips: string[]): Promise<void> {
        await apiClient.delete('/scanner/blacklist', { data: { ips } });
    },
    async deleteBlacklistByIp(ip: string): Promise<void> {
        await apiClient.delete(`/scanner/blacklist/${encodeURIComponent(ip)}`);
    }
};

export const FrpcAPI = {
    async getStatus(): Promise<{ initialized: boolean; platform: string; running: boolean; pid: number | null; config_path: string; defaults: { local_port: string } }> {
        const res = await apiClient.get('/frpc/status');
        return res.data.data;
    },
    async getOverview(limit = 200): Promise<{ tcp: FrpcTcpItem[]; logs: string[] }> {
        const res = await apiClient.get('/frpc/overview', { params: { limit } });
        return res.data.data;
    },
    async getWebStatus(): Promise<{ tcp: FrpcTcpItem[] }> {
        const res = await apiClient.get('/frpc/web-status');
        return res.data.data;
    },
    async getConfig(): Promise<string> {
        const res = await apiClient.get('/frpc/config');
        return res.data.data.content as string;
    },
    async saveConfig(content: string): Promise<void> {
        await apiClient.post('/frpc/config', { content });
    },
    async start(): Promise<{ pid: number }> {
        const res = await apiClient.post('/frpc/start');
        return res.data.data;
    },
    async stop(): Promise<void> {
        await apiClient.post('/frpc/stop');
    },
    async getLogs(limit = 200): Promise<string[]> {
        const res = await apiClient.get('/frpc/logs', { params: { limit } });
        return res.data.data as string[];
    },
    async clearLogs(): Promise<void> {
        await apiClient.delete('/frpc/logs');
    },
    async poll(cursor?: number): Promise<FrpcPollPayload> {
        const res = await apiClient.get('/frpc/poll', {
            params: typeof cursor === 'number' ? { cursor } : undefined,
        });
        return res.data.data;
    }
};

export const CloudflaredAPI = {
    async getStatus(): Promise<{ initialized: boolean; platform: string; running: boolean; pid: number | null }> {
        const res = await apiClient.get('/cloudflared/status');
        return res.data.data;
    },
    async getConfig(): Promise<{ token: string }> {
        const res = await apiClient.get('/cloudflared/config');
        return res.data.data;
    },
    async saveConfig(token: string): Promise<void> {
        await apiClient.post('/cloudflared/config', { token });
    },
    async start(): Promise<{ pid: number }> {
        const res = await apiClient.post('/cloudflared/start');
        return res.data.data;
    },
    async stop(): Promise<void> {
        await apiClient.post('/cloudflared/stop');
    },
    async getLogs(limit = 200): Promise<string[]> {
        const res = await apiClient.get('/cloudflared/logs', { params: { limit } });
        return res.data.data as string[];
    },
    async clearLogs(): Promise<void> {
        await apiClient.delete('/cloudflared/logs');
    },
    async poll(cursor?: number): Promise<CloudflaredPollPayload> {
        const res = await apiClient.get('/cloudflared/poll', {
            params: typeof cursor === 'number' ? { cursor } : undefined,
        });
        return res.data.data;
    }
};

export type PollTarget = 'dashboard' | 'ddns' | 'frpc' | 'cloudflared';

export type PollingPayloadMap = {
    dashboard: TrafficStats;
    ddns: DDNSPollPayload;
    frpc: FrpcPollPayload;
    cloudflared: CloudflaredPollPayload;
};

export const PollingAPI = {
    async poll<T extends PollTarget>(target: T, cursor?: number): Promise<PollingPayloadMap[T]> {
        switch (target) {
            case 'dashboard':
                return await DashboardAPI.getRealtime() as PollingPayloadMap[T];
            case 'ddns':
                return await DDNSAPI.poll(cursor) as PollingPayloadMap[T];
            case 'frpc':
                return await FrpcAPI.poll(cursor) as PollingPayloadMap[T];
            case 'cloudflared':
                return await CloudflaredAPI.poll(cursor) as PollingPayloadMap[T];
            default:
                throw new Error(`Unsupported poll target: ${String(target)}`);
        }
    },
};

export const SessionAPI = {
    async list(): Promise<SessionRecord[]> {
        const res = await apiClient.get('/sessions');
        return res.data.data;
    },
    async get(id: string): Promise<SessionRecord> {
        const res = await apiClient.get(`/sessions/${encodeURIComponent(id)}`);
        return res.data.data;
    },
    async getMobility(id: string): Promise<SessionMobilityDetails> {
        const res = await apiClient.get(`/sessions/${encodeURIComponent(id)}/mobility`);
        return res.data.data;
    },
    async kick(id: string): Promise<void> {
        await apiClient.delete(`/sessions/${encodeURIComponent(id)}`);
    }
};

export type BackoffItem = {
    ip: string;
    attempts: number;
    blocked: boolean;
    retryAfter?: number;
    blockedUntil?: number;
};

export const BackoffAPI = {
    async list(): Promise<BackoffItem[]> {
        const res = await apiClient.get('/backoff/list');
        return res.data.data || [];
    },
    async status(ip: string): Promise<BackoffItem> {
        const res = await apiClient.get('/backoff/status', { params: { ip } });
        return res.data.data;
    },
    async reset(ip: string): Promise<void> {
        await apiClient.post('/backoff/reset', { ip });
    }
};

export const AcmeAPI = {
    async dnsProviders(): Promise<Array<{ dnsType: string; label: string; group: string; envKeys: string[] }>> {
        const res = await apiClient.get('/acme/dns-providers');
        return res.data.data || [];
    },
    async status(): Promise<{ status: 'uninstalled' | 'installing' | 'installed' | 'error'; progress: number; message: string; acmeCert?: { primaryDomain: string; info: any } | null }> {
        const res = await apiClient.get('/acme/status');
        return res.data.data;
    },
    async getConfig(): Promise<{ domains: string[]; dnsType: string; credentials: Record<string, string>; updatedAt: string } | null> {
        const res = await apiClient.get('/acme/config');
        return res.data.data || null;
    },
    async saveConfig(payload: { domains: string[]; dnsType: string; credentials?: Record<string, string> }): Promise<{ domains: string[]; dnsType: string; credentials: Record<string, string>; updatedAt: string }> {
        const res = await apiClient.post('/acme/config', payload);
        return res.data.data;
    },
    async init(): Promise<void> {
        await apiClient.post('/acme/init');
    },
    async uninstall(): Promise<void> {
        await apiClient.delete('/acme');
    },
    async request(payload: { domains: string[]; dnsType: string; credentials?: Record<string, string> }): Promise<{ jobId: string }> {
        const res = await apiClient.post('/acme/request', payload);
        return res.data.data;
    },
    async job(id: string): Promise<{ id: string; domains: string[]; method: string; provider: string | null; status: string; progress: number; message?: string }> {
        const res = await apiClient.get(`/acme/jobs/${encodeURIComponent(id)}`);
        return res.data.data;
    },
    async logs(id: string): Promise<string[]> {
        const res = await apiClient.get(`/acme/jobs/${encodeURIComponent(id)}/logs`);
        return res.data.data || [];
    },
    async poll(id: string, opts?: { limit?: number; order?: 'asc' | 'desc' }): Promise<{ job: { id: string; domains: string[]; method: string; provider: string | null; status: string; progress: number; message?: string }; logs: string[]; analysis?: { reason: "dns_credentials_invalid" | "dns_credentials_invalid_email" | "dns_api_rate_limited" | "acme_frequency_limited" | "unknown"; provider?: string; message: string; evidence?: string[] } | null }> {
        const res = await apiClient.get(`/acme/jobs/${encodeURIComponent(id)}/poll`, { params: { limit: opts?.limit, order: opts?.order } });
        return res.data.data;
    },
    async certInfo(domain: string): Promise<{ domain: string; info: any }> {
        const res = await apiClient.get(`/acme/certs/${encodeURIComponent(domain)}`);
        return res.data.data;
    },
    async download(domain: string): Promise<Blob> {
        const res = await apiClient.get(`/acme/certs/${encodeURIComponent(domain)}/download`, { responseType: 'blob' });
        return res.data;
    },
    async deploy(domain: string): Promise<void> {
        await apiClient.post(`/acme/certs/${encodeURIComponent(domain)}/deploy`);
    },
    async deleteCert(domain: string): Promise<void> {
        await apiClient.delete(`/acme/certs/${encodeURIComponent(domain)}`);
    }
};

export const DashboardAPI = {
    async getStats(rangeSec: number, userId?: string): Promise<DashboardStats> {
        const res = await apiClient.get('/dashboard/stats', { params: { rangeSec, userId } });
        return res.data.data;
    },
    async getRealtime(): Promise<TrafficStats> {
        const res = await apiClient.get('/dashboard/realtime');
        return res.data.data;
    }
};

export interface DiscoveredServiceInfo {
    port: number;
    httpStatus: number;
    detail: {
        name: string;
        label: string;
        rule: {
            path: string;
            rewrite_html: boolean;
            use_auth: boolean;
            use_root_mode: boolean;
            strip_path: boolean;
            target: string;
        };
        isDefault: boolean;
    }
}

export interface ScanDiscoverResponse {
    host: string;
    totalPortsScanned: number;
    foundServices: number;
    services: DiscoveredServiceInfo[];
}

export const ScanAPI = {
    async discover(): Promise<ScanDiscoverResponse> {
        const res = await apiClient.get('/scan/discover');
        return res.data.data;
    }
};

export type DDNSLogEntry = {
    time: string;
    level: 'info' | 'error' | 'warn';
    message: string;
};

export type DDNSStatusPayload = {
    enabled: boolean;
    provider: string | null;
    updateScope: 'dual_stack' | 'ipv6_only' | 'ipv4_only';
    networkInterface: string;
    lastIP: {
        ipv4: string | null;
        ipv6: string | null;
        updated_at: string | null;
    };
    lastCheck: {
        checked_at: string | null;
        outcome: 'updated' | 'noop' | 'skipped' | 'error' | null;
        message: string | null;
    };
};

export type DDNSNetworkInterfacePayload = {
    name: string;
    label: string;
    summary: string;
    hasIpv4: boolean;
    hasIpv6: boolean;
    addresses: Array<{
        family: 'ipv4' | 'ipv6';
        address: string;
        cidr: string | null;
        internal: boolean;
    }>;
};

export type DDNSPollPayload = {
    cursor: number;
    reset: boolean;
    logs: DDNSLogEntry[];
    status: DDNSStatusPayload;
};

export type FrpcTcpItem = {
    name: string;
    type: string;
    status: string;
    err: string;
    local_addr: string;
    plugin: string;
    remote_addr: string;
};

export type FrpcStatusPayload = {
    running: boolean;
    pid: number | null;
    tcp: FrpcTcpItem[];
};

export type FrpcPollPayload = {
    cursor: number;
    reset: boolean;
    logs: string[];
    status: FrpcStatusPayload;
};

export type CloudflaredStatusPayload = {
    running: boolean;
    pid: number | null;
};

export type CloudflaredPollPayload = {
    cursor: number;
    reset: boolean;
    logs: string[];
    status: CloudflaredStatusPayload;
};

export const DDNSAPI = {
    async getStatus(): Promise<DDNSStatusPayload> {
        const res = await apiClient.get('/ddns/status');
        return res.data.data;
    },
    async toggle(enabled: boolean): Promise<void> {
        await apiClient.post('/ddns/toggle', { enabled });
    },
    async getProviders(): Promise<Array<{ name: string; label: string; fields: Array<{ key: string; label: string; type: string; placeholder?: string; required?: boolean; options?: Array<{ label: string; value: string }>; description?: string }> }>> {
        const res = await apiClient.get('/ddns/providers');
        return res.data.data;
    },
    async getNetworkInterfaces(): Promise<DDNSNetworkInterfacePayload[]> {
        const res = await apiClient.get('/ddns/interfaces');
        return res.data.data;
    },
    async setProvider(provider: string): Promise<void> {
        await apiClient.post('/ddns/provider', { provider });
    },
    async getConfig(provider: string): Promise<Record<string, string>> {
        const res = await apiClient.get(`/ddns/config/${encodeURIComponent(provider)}`);
        return res.data.data;
    },
    async saveConfig(provider: string, config: Record<string, string>): Promise<void> {
        await apiClient.post(`/ddns/config/${encodeURIComponent(provider)}`, { config });
    },
    async test(): Promise<{ success: boolean; message: string; data?: { ipv4: string | null; ipv6: string | null } }> {
        const res = await apiClient.post('/ddns/test');
        return res.data;
    },
    async getLogs(limit = 200): Promise<DDNSLogEntry[]> {
        const res = await apiClient.get('/ddns/logs', { params: { limit } });
        return res.data.data;
    },
    async clearLogs(): Promise<void> {
        await apiClient.delete('/ddns/logs');
    },
    async poll(cursor?: number): Promise<DDNSPollPayload> {
        const res = await apiClient.get('/ddns/poll', {
            params: typeof cursor === 'number' ? { cursor } : undefined,
        });
        return res.data.data;
    }
};
