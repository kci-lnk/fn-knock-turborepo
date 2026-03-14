import Redis from 'ioredis';
import { X509Certificate, createPrivateKey, randomBytes } from 'node:crypto';
import { homedir } from 'node:os';
import { spawn } from "node:child_process";
import { dataPath } from './AppDirManager';
import { ACME_EXECUTABLE_PATH, ACME_HOME_DIR } from './acme-paths';
import { DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, RedisLogBuffer } from './redis-log-buffer';
import { collectStreamOutput, fileExists, waitForProcessExit } from "./runtime";

const REDIS_CONFIG = {
    host: process.env.REDIS_HOST || '127.0.0.1',
    port: parseInt(process.env.REDIS_PORT || '6379'),
    password: process.env.REDIS_PASSWORD,
};

export const redis = new Redis(REDIS_CONFIG);
redis.on('error', (err) => {
    console.error('Redis connection error:', err);
});

export interface ProxyMapping {
    path: string;
    target: string;
    rewrite_html: boolean;
    use_auth: boolean;
    use_root_mode: boolean;
    strip_path: boolean;
}

export interface SSLConfig {
    cert: string;
    key: string;
}

export interface SSLCertInfo {
    issuer: string;
    subject: string;
    validFrom: string;
    validTo: string;
    dnsNames: string[];
    serialNumber: string;
}

export interface SSLStatus {
    enabled: boolean;
    certInfo?: SSLCertInfo;
}

export interface FnosShareBypassConfig {
    enabled: boolean;
    upstream_timeout_ms: number;
    validation_cache_ttl_seconds: number;
    validation_lock_ttl_seconds: number;
    session_ttl_seconds: number;
}

export type CaptchaProvider = 'pow' | 'turnstile';

export type CaptchaWidgetMode = 'normal';

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

export type AcmeJobStatus = 'queued' | 'running' | 'succeeded' | 'failed';
export type AcmeJobMethod = 'dns' | 'http' | 'https';
export type AcmeJob = {
    id: string;
    domains: string[];
    method: AcmeJobMethod;
    provider: string | null;
    createdAt: string;
    status: AcmeJobStatus;
    progress: number;
    message?: string;
};

export type AcmeSettings = {
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    updatedAt: string;
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

export interface AppConfig {
    run_type: 0 | 1;
    whitelist_ips: string[];
    proxy_mappings: ProxyMapping[];
    ssl: SSLConfig;
    default_route: string;
    default_tunnel?: 'frp' | 'cloudflared';
    fnos_share_bypass?: FnosShareBypassConfig;
}

export interface RunModePromptPreferences {
    directToReverseProxy: boolean;
    reverseProxyToDirect: boolean;
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

const DEFAULT_CONFIG: AppConfig = {
    run_type: 1,
    whitelist_ips: [],
    proxy_mappings: [],
    ssl: {
        cert: '',
        key: ''
    },
    default_route: '/__select__',
    default_tunnel: 'frp',
    fnos_share_bypass: {
        enabled: false,
        upstream_timeout_ms: 2500,
        validation_cache_ttl_seconds: 30,
        validation_lock_ttl_seconds: 5,
        session_ttl_seconds: 300,
    },
};

const DEFAULT_RUN_MODE_PROMPT_PREFERENCES: RunModePromptPreferences = {
    directToReverseProxy: false,
    reverseProxyToDirect: false,
};

const DEFAULT_FNOS_SHARE_BYPASS_CONFIG: FnosShareBypassConfig = {
    enabled: false,
    upstream_timeout_ms: 2500,
    validation_cache_ttl_seconds: 30,
    validation_lock_ttl_seconds: 5,
    session_ttl_seconds: 300,
};

const DEFAULT_CAPTCHA_SETTINGS: CaptchaSettings = {
    provider: 'pow',
    widget_mode: 'normal',
    pow: {},
    turnstile: {
        site_key: '',
        secret_key: '',
    },
};

const normalizePositiveInt = (
    value: unknown,
    fallback: number,
    { min = 1, max = Number.MAX_SAFE_INTEGER }: { min?: number; max?: number } = {},
): number => {
    const parsed = Number.parseInt(String(value ?? ''), 10);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.min(max, Math.max(min, parsed));
};

const normalizeFnosShareBypassConfig = (
    value?: Partial<FnosShareBypassConfig> | null,
): FnosShareBypassConfig => {
    const raw = value ?? {};

    return {
        enabled: raw.enabled === true,
        upstream_timeout_ms: normalizePositiveInt(
            raw.upstream_timeout_ms,
            DEFAULT_FNOS_SHARE_BYPASS_CONFIG.upstream_timeout_ms,
            { min: 500, max: 15000 },
        ),
        validation_cache_ttl_seconds: normalizePositiveInt(
            raw.validation_cache_ttl_seconds,
            DEFAULT_FNOS_SHARE_BYPASS_CONFIG.validation_cache_ttl_seconds,
            { min: 5, max: 300 },
        ),
        validation_lock_ttl_seconds: normalizePositiveInt(
            raw.validation_lock_ttl_seconds,
            DEFAULT_FNOS_SHARE_BYPASS_CONFIG.validation_lock_ttl_seconds,
            { min: 1, max: 30 },
        ),
        session_ttl_seconds: normalizePositiveInt(
            raw.session_ttl_seconds,
            DEFAULT_FNOS_SHARE_BYPASS_CONFIG.session_ttl_seconds,
            { min: 30, max: 3600 },
        ),
    };
};

const normalizeCaptchaSettings = (
    value?: Partial<CaptchaSettings> | null,
): CaptchaSettings => {
    const raw = value ?? {};
    const provider = raw.provider === 'turnstile' ? 'turnstile' : 'pow';
    const turnstileRaw = raw.turnstile ?? {};

    return {
        provider,
        widget_mode: 'normal',
        pow: {},
        turnstile: {
            site_key: typeof turnstileRaw.site_key === 'string' ? turnstileRaw.site_key.trim() : '',
            secret_key: typeof turnstileRaw.secret_key === 'string' ? turnstileRaw.secret_key.trim() : '',
        },
    };
};

export class ConfigManager {
    private redis: Redis;
    private configKey = 'fn_knock:config';
    private captchaSettingsKey = 'fn_knock:captcha:settings';
    private caHostsKey = 'fn_knock:ca:hosts';
    private acmeJobKey = 'fn_knock:acme:job:';
    private acmeLogsKey = 'fn_knock:acme:logs:';
    private acmeCertKey = 'fn_knock:acme:cert:';
    private acmeSettingsKey = 'fn_knock:acme:settings';
    private onboardingCompletedKey = 'fn_knock:onboarding:completed';
    private runModePromptPreferencesKey = 'fn_knock:run-mode:prompt-preferences';

    constructor() {
        this.redis = redis;
    }

    async getConfig(): Promise<AppConfig> {
        try {
            const data = await this.redis.get(this.configKey);
            if (data) {
                // 处理已有数据缺少 default_route 的兼容情况
                const parsed = JSON.parse(data) as AppConfig;
                if (!parsed.default_route) parsed.default_route = '/__select__';
                if (!parsed.default_tunnel) parsed.default_tunnel = 'frp';
                parsed.fnos_share_bypass = normalizeFnosShareBypassConfig(parsed.fnos_share_bypass);
                return parsed;
            }
        } catch (e) {
            console.error("Failed to parse config from redis", e);
        }
        return {
            ...DEFAULT_CONFIG,
            fnos_share_bypass: { ...DEFAULT_FNOS_SHARE_BYPASS_CONFIG },
        };
    }

    /**
     * 返回不含 SSL cert/key 原文的配置（供 /api/admin/config 使用）
     */
    async getConfigSafe(): Promise<any> {
        const config = await this.getConfig();
        const { ssl, ...rest } = config;
        return {
            ...rest,
            ssl: { enabled: !!(ssl.cert && ssl.key) }
        };
    }

    /**
     * 解析 X.509 证书，返回结构化信息
     */
    private parseCertInfo(certPem: string): SSLCertInfo | null {
        try {
            const x509 = new X509Certificate(certPem);
            // 解析 DNS Names (以及 IP) from subjectAltName
            const sanStr = x509.subjectAltName || '';
            const dnsNames: string[] = [];
            sanStr
                .split(',')
                .map(s => s.trim())
                .filter(Boolean)
                .forEach(entry => {
                    const idx = entry.indexOf(':');
                    if (idx <= 0) return;
                    const label = entry.slice(0, idx).trim().toLowerCase();
                    const value = entry.slice(idx + 1).trim();
                    // 常见标签：DNS、IP、IP Address（Node/openssl 输出可能不同）
                    if (label === 'dns' || label === 'ip' || label === 'ip address') {
                        dnsNames.push(value);
                    }
                });

            return {
                issuer: x509.issuer,
                subject: x509.subject,
                validFrom: x509.validFrom,
                validTo: x509.validTo,
                dnsNames,
                serialNumber: x509.serialNumber
            };
        } catch (e) {
            console.error('Failed to parse X.509 certificate:', e);
            return null;
        }
    }

    /**
     * 获取 SSL 状态和证书结构化信息
     */
    async getSSLStatus(): Promise<SSLStatus> {
        const config = await this.getConfig();
        const { ssl } = config;
        if (!ssl.cert || !ssl.key) {
            return { enabled: false };
        }
        const certInfo = this.parseCertInfo(ssl.cert);
        return {
            enabled: true,
            certInfo: certInfo || undefined
        };
    }

    /**
     * 验证 SSL 证书和私钥是否合法
     */
    validateSSLCert(cert: string, key: string): { valid: boolean; error?: string } {
        try {
            new X509Certificate(cert);
        } catch (e: any) {
            return { valid: false, error: `证书格式无效: ${e.message}` };
        }
        try {
            createPrivateKey(key);
        } catch (e: any) {
            return { valid: false, error: `私钥格式无效: ${e.message}` };
        }
        // Check cert-key match
        try {
            const x509 = new X509Certificate(cert);
            const privateKey = createPrivateKey(key);
            if (!x509.checkPrivateKey(privateKey)) {
                return { valid: false, error: '证书与私钥不匹配' };
            }
        } catch (e: any) {
            return { valid: false, error: `证书与私钥校验失败: ${e.message}` };
        }
        return { valid: true };
    }

    /**
     * 清除 SSL 配置
     */
    async clearSSL(): Promise<void> {
        const config = await this.getConfig();
        config.ssl = { cert: '', key: '' };
        await this.saveConfig(config);
    }

    async saveConfig(config: AppConfig): Promise<void> {
        await this.redis.set(this.configKey, JSON.stringify(config));
    }

    async createAcmeJob(job: AcmeJob): Promise<void> {
        const key = `${this.acmeJobKey}${job.id}`;
        await this.redis.set(key, JSON.stringify(job), 'EX', 86400);
    }

    async updateAcmeJob(id: string, patch: Partial<AcmeJob>): Promise<void> {
        const key = `${this.acmeJobKey}${id}`;
        const raw = await this.redis.get(key);
        if (!raw) return;
        let obj: AcmeJob;
        try {
            obj = JSON.parse(raw) as AcmeJob;
        } catch {
            return;
        }
        const next = { ...obj, ...patch };
        await this.redis.set(key, JSON.stringify(next), 'EX', 86400);
    }

    async getAcmeJob(id: string): Promise<AcmeJob | null> {
        const raw = await this.redis.get(`${this.acmeJobKey}${id}`);
        if (!raw) return null;
        try {
            return JSON.parse(raw) as AcmeJob;
        } catch {
            return null;
        }
    }

    async appendAcmeLog(jobId: string, line: string): Promise<void> {
        const key = `${this.acmeLogsKey}${jobId}`;
        const buffer = new RedisLogBuffer(this.redis, {
            key,
            ttlSeconds: 86400,
            maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
        });
        await buffer.append([line]);
    }

    async clearAcmeLogs(jobId: string): Promise<void> {
        await this.redis.del(`${this.acmeLogsKey}${jobId}`);
    }

    async getAcmeLogs(jobId: string, limit: number = 500, order: 'asc' | 'desc' = 'asc'): Promise<string[]> {
        const key = `${this.acmeLogsKey}${jobId}`;
        const len = await this.redis.llen(key);
        if (len === 0) return [];
        const start = Math.max(0, len - limit);
        const arr = await this.redis.lrange(key, start, -1);
        return order === 'desc' ? arr.reverse() : arr;
    }

    async saveAcmeSettings(value: Omit<AcmeSettings, 'updatedAt'>): Promise<AcmeSettings> {
        const next: AcmeSettings = { ...value, updatedAt: new Date().toISOString() };
        await this.redis.set(this.acmeSettingsKey, JSON.stringify(next));
        return next;
    }

    async getAcmeSettings(): Promise<AcmeSettings | null> {
        const raw = await this.redis.get(this.acmeSettingsKey);
        if (!raw) return null;
        try {
            const obj = JSON.parse(raw);
            if (!obj || typeof obj !== 'object') return null;
            if (!Array.isArray(obj.domains) || typeof obj.dnsType !== 'string' || typeof obj.credentials !== 'object') return null;
            return obj as AcmeSettings;
        } catch {
            return null;
        }
    }

    async saveAcmeCert(domain: string, cert: string, keyPem: string): Promise<void> {
        const k = `${this.acmeCertKey}${domain}`;
        await this.redis.set(k, JSON.stringify({ cert, key: keyPem }));
    }

    async getAcmeCert(domain: string): Promise<{ cert: string; key: string } | null> {
        const raw = await this.redis.get(`${this.acmeCertKey}${domain}`);
        if (!raw) return null;
        try {
            const obj = JSON.parse(raw);
            if (
                typeof obj?.cert === 'string' &&
                typeof obj?.key === 'string' &&
                obj.cert.trim() &&
                obj.key.trim()
            ) {
                return obj;
            }
            return null;
        } catch {
            return null;
        }
    }

    async deleteAcmeCert(domain: string): Promise<void> {
        await this.redis.del(`${this.acmeCertKey}${domain}`);
    }

    async getAcmeCertInfo(domain: string): Promise<SSLCertInfo | null> {
        const pair = await this.getAcmeCert(domain);
        if (!pair) return null;
        return this.parseCertInfo(pair.cert);
    }

    async saveAcmeCertFromFS(domain: string, opts?: { forceInstall?: boolean }): Promise<boolean> {
        const { join } = await import('node:path');
        const { promises: fs } = await import('node:fs');

        const domainDir = join(dataPath, 'ssl', domain);
        const installedKeyPath = join(domainDir, `${domain}.key`);
        const installedFullchainPath = join(domainDir, 'fullchain.cer');

        try {
            const hasKey = await fileExists(installedKeyPath);
            const hasFullchain = await fileExists(installedFullchainPath);
            const shouldInstall = !!opts?.forceInstall || !hasKey || !hasFullchain;

            if (shouldInstall) {
                await fs.mkdir(domainDir, { recursive: true });
                const exists = await fileExists(ACME_EXECUTABLE_PATH);
                if (!exists) return false;

                const installProc = spawn(ACME_EXECUTABLE_PATH, [
                    '--home',
                    ACME_HOME_DIR,
                    '--config-home',
                    ACME_HOME_DIR,
                    '--install-cert',
                    '-d',
                    domain,
                    '--key-file',
                    installedKeyPath,
                    '--fullchain-file',
                    installedFullchainPath,
                ], { stdio: ['ignore', 'pipe', 'pipe'] });
                const installExitPromise = waitForProcessExit(installProc);

                const [, , exitCode] = await Promise.all([
                    collectStreamOutput(installProc.stdout).catch(() => ''),
                    collectStreamOutput(installProc.stderr).catch(() => ''),
                    installExitPromise,
                ]);
                if (exitCode !== 0) return false;
            }

            const cert = await fs.readFile(installedFullchainPath, 'utf-8');
            const key = await fs.readFile(installedKeyPath, 'utf-8');
            if (!cert.trim() || !key.trim()) return false;
            if (!this.parseCertInfo(cert)) return false;
            await this.saveAcmeCert(domain, cert, key);
            return true;
        } catch {
            try {
                const fallbackHomes = [ACME_HOME_DIR, join(homedir(), '.acme.sh')];
                for (const home of fallbackHomes) {
                    const certDir = join(home, domain);
                    const certPathA = join(certDir, 'fullchain.cer');
                    const certPathB = join(certDir, `${domain}.cer`);
                    const keyPath = join(certDir, `${domain}.key`);
                    try {
                        const cert = await fs.readFile(certPathA, 'utf-8').catch(async () => await fs.readFile(certPathB, 'utf-8'));
                        const key = await fs.readFile(keyPath, 'utf-8');
                        if (!cert.trim() || !key.trim()) continue;
                        if (!this.parseCertInfo(cert)) continue;
                        await this.saveAcmeCert(domain, cert, key);
                        return true;
                    } catch {
                        // try next fallback directory
                    }
                }
                return false;
            } catch {
                return false;
            }
        }
    }

    async updateRunType(run_type: 0 | 1): Promise<void> {
        const config = await this.getConfig();
        config.run_type = run_type;
        await this.saveConfig(config);
    }

    async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
        const raw = await this.redis.get(this.runModePromptPreferencesKey);
        if (!raw) return DEFAULT_RUN_MODE_PROMPT_PREFERENCES;

        try {
            const parsed = JSON.parse(raw) as Partial<RunModePromptPreferences>;
            return {
                directToReverseProxy: parsed.directToReverseProxy === true,
                reverseProxyToDirect: parsed.reverseProxyToDirect === true,
            };
        } catch {
            return DEFAULT_RUN_MODE_PROMPT_PREFERENCES;
        }
    }

    async updateRunModePromptPreferences(
        patch: Partial<RunModePromptPreferences>,
    ): Promise<RunModePromptPreferences> {
        const next = {
            ...(await this.getRunModePromptPreferences()),
            ...patch,
        };
        await this.redis.set(this.runModePromptPreferencesKey, JSON.stringify(next));
        return next;
    }

    async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
        const config = await this.getConfig();
        return normalizeFnosShareBypassConfig(config.fnos_share_bypass);
    }

    async updateFnosShareBypassConfig(
        patch: Partial<FnosShareBypassConfig>,
    ): Promise<FnosShareBypassConfig> {
        const config = await this.getConfig();
        const next = normalizeFnosShareBypassConfig({
            ...config.fnos_share_bypass,
            ...patch,
        });
        config.fnos_share_bypass = next;
        await this.saveConfig(config);
        return next;
    }

    async getCaptchaSettings(): Promise<CaptchaSettings> {
        const raw = await this.redis.get(this.captchaSettingsKey);
        if (!raw) return DEFAULT_CAPTCHA_SETTINGS;

        try {
            const parsed = JSON.parse(raw) as Partial<CaptchaSettings>;
            return normalizeCaptchaSettings(parsed);
        } catch {
            return DEFAULT_CAPTCHA_SETTINGS;
        }
    }

    async updateCaptchaSettings(
        patch: Partial<CaptchaSettings>,
    ): Promise<CaptchaSettings> {
        const current = await this.getCaptchaSettings();
        const next = normalizeCaptchaSettings({
            ...current,
            ...patch,
            turnstile: {
                ...current.turnstile,
                ...(patch.turnstile ?? {}),
            },
        });
        await this.redis.set(this.captchaSettingsKey, JSON.stringify(next));
        return next;
    }

    async updateProxyMappings(mappings: ProxyMapping[]): Promise<void> {
         const config = await this.getConfig();
         config.proxy_mappings = mappings;
         await this.saveConfig(config);
    }

    async updateSSLConfig(ssl: SSLConfig): Promise<void> {
         const config = await this.getConfig();
         config.ssl = ssl;
         await this.saveConfig(config);
    }

    async addIPBackoff(ip: string, ttlSeconds: number): Promise<void> {
         await this.redis.set(`fn_knock:backoff:${ip}`, '1', 'EX', ttlSeconds);
    }
    
    async getIPBackoff(ip: string): Promise<boolean> {
        const val = await this.redis.get(`fn_knock:backoff:${ip}`);
        return val !== null;
    }
    
    async addNonce(nonce: string, ttlSeconds: number = 300): Promise<void> {
        await this.redis.set(`fn_knock:nonce:${nonce}`, '1', 'EX', ttlSeconds);
    }

    /**
     * Stores a nonce if it doesn't exist. Returns true if it was set (new nonce), false if it already exists.
     */
    async setNonceIfNotExists(nonce: string, ttlSeconds: number = 600): Promise<boolean> {
        const key = `fn_knock:nonce:${nonce}`;
        const result = await this.redis.set(key, '1', 'EX', ttlSeconds, 'NX');
        return result === 'OK';
    }

    /**
     * Stores a cron/distributed lock if it doesn't exist. Returns true when lock is acquired.
     */
    async setLockIfNotExists(lockName: string, ttlSeconds: number = 600): Promise<boolean> {
        const key = `fn_knock:lock:${lockName}`;
        const result = await this.redis.set(key, '1', 'EX', ttlSeconds, 'NX');
        return result === 'OK';
    }

    async updateDefaultRoute(route: string): Promise<void> {
         const config = await this.getConfig();
         config.default_route = route;
         await this.saveConfig(config);
    }

    async updateDefaultTunnel(tunnel: 'frp' | 'cloudflared'): Promise<void> {
        const config = await this.getConfig();
        config.default_tunnel = tunnel;
        await this.saveConfig(config);
    }

    async getOnboardingStatus(): Promise<{ completed: boolean }> {
        const value = await this.redis.get(this.onboardingCompletedKey);
        return { completed: value === '1' };
    }

    async markOnboardingCompleted(): Promise<void> {
        await this.redis.set(this.onboardingCompletedKey, '1');
    }

    // CA Hosts list in Redis
    async getCAHosts(): Promise<string[]> {
        const raw = await this.redis.get(this.caHostsKey);
        if (!raw) return [];
        try {
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed)) return parsed.filter(x => typeof x === 'string');
        } catch {}
        return [];
    }

    async saveCAHosts(hosts: string[]): Promise<void> {
        await this.redis.set(this.caHostsKey, JSON.stringify(hosts));
    }

    async addCAHost(value: string): Promise<string[]> {
        const v = value.trim();
        if (!v) return await this.getCAHosts();
        const hosts = await this.getCAHosts();
        if (!hosts.includes(v)) {
            hosts.push(v);
            await this.saveCAHosts(hosts);
        }
        return hosts;
    }

    async removeCAHost(value: string): Promise<string[]> {
        const v = value.trim();
        const hosts = await this.getCAHosts();
        const next = hosts.filter(h => h !== v);
        if (next.length !== hosts.length) {
            await this.saveCAHosts(next);
        }
        return next;
    }

    async clearCAHosts(): Promise<void> {
        await this.saveCAHosts([]);
    }

    // TOTP secret management
    private totpKey = 'fn_knock:totp_secret';
    private totpListKey = 'fn_knock:totps';
    private passkeyListKey = 'fn_knock:passkeys';
    private passkeyChallengeKey = 'fn_knock:passkey:challenge';
    private passkeyBindKey = 'fn_knock:passkey:bind';

    async getTOTPCredentials(): Promise<TOTPCredential[]> {
        const raw = await this.redis.get(this.totpListKey);
        if (!raw) {
            // Migration for old single secret
            const oldSecret = await this.redis.get(this.totpKey);
            if (oldSecret) {
                const legacyTotp: TOTPCredential = {
                    id: 'legacy-totp-id',
                    secret: oldSecret,
                    comment: '默认凭据',
                    createdAt: new Date().toISOString()
                };
                await this.saveTOTPCredentials([legacyTotp]);
                await this.redis.del(this.totpKey);
                const passkeys = await this.getPasskeys();
                let passkeysModified = false;
                for (const pk of passkeys) {
                    if (!pk.totpId) {
                        pk.totpId = legacyTotp.id;
                        passkeysModified = true;
                    }
                }
                if (passkeysModified) await this.savePasskeys(passkeys);
                return [legacyTotp];
            }
            return [];
        }
        try {
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed)) return parsed as TOTPCredential[];
        } catch {
            return [];
        }
        return [];
    }

    async saveTOTPCredentials(totps: TOTPCredential[]): Promise<void> {
        await this.redis.set(this.totpListKey, JSON.stringify(totps));
    }

    async addTOTPCredential(totp: TOTPCredential): Promise<void> {
        const totps = await this.getTOTPCredentials();
        totps.push(totp);
        await this.saveTOTPCredentials(totps);
    }

    async updateTOTPCredential(id: string, comment: string): Promise<boolean> {
        const totps = await this.getTOTPCredentials();
        const target = totps.find(t => t.id === id);
        if (!target) return false;
        target.comment = comment;
        await this.saveTOTPCredentials(totps);
        return true;
    }

    async deleteTOTPCredential(id: string): Promise<boolean> {
        const totps = await this.getTOTPCredentials();
        const updated = totps.filter(t => t.id !== id);
        if (updated.length === totps.length) return false;
        await this.saveTOTPCredentials(updated);
        
        // Cascade delete passkeys
        const passkeys = await this.getPasskeys();
        const remainingPasskeys = passkeys.filter(pk => pk.totpId !== id);
        if (remainingPasskeys.length !== passkeys.length) {
            await this.savePasskeys(remainingPasskeys);
        }
        return true;
    }

    // Session management
    async addSession(sessionId: string, session: LoginSession, ttlSeconds: number): Promise<void> {
        await this.redis.set(`fn_knock:session:${sessionId}`, JSON.stringify(session), 'EX', ttlSeconds);
    }

    async getSession(sessionId: string): Promise<LoginSession | null> {
        const raw = await this.redis.get(`fn_knock:session:${sessionId}`);
        if (!raw) return null;
        try {
            const data = JSON.parse(raw) as LoginSession;
            return data;
        } catch {
            return null;
        }
    }

    async deleteSession(sessionId: string): Promise<void> {
        await this.redis.del(`fn_knock:session:${sessionId}`);
    }

    async updateSession(sessionId: string, updates: Partial<LoginSession>): Promise<LoginSession | null> {
        const key = `fn_knock:session:${sessionId}`;
        const [raw, ttl] = await Promise.all([
            this.redis.get(key),
            this.redis.ttl(key)
        ]);
        if (!raw) return null;

        try {
            const current = JSON.parse(raw) as LoginSession;
            const next: LoginSession = {
                ...current,
                ...updates
            };

            if (ttl > 0) {
                await this.redis.set(key, JSON.stringify(next), 'EX', ttl);
            } else {
                await this.redis.set(key, JSON.stringify(next));
            }
            return next;
        } catch {
            return null;
        }
    }

    async isValidSession(sessionId: string): Promise<boolean> {
        const val = await this.redis.get(`fn_knock:session:${sessionId}`);
        return val !== null;
    }

    async listSessions(): Promise<Array<{ id: string; data: LoginSession }>> {
        const match = 'fn_knock:session:*';
        let cursor = '0';
        const keys: string[] = [];
        do {
            const res = await this.redis.scan(cursor, 'MATCH', match, 'COUNT', 100);
            cursor = res[0];
            const batch = res[1] as string[];
            if (batch && batch.length) keys.push(...batch);
        } while (cursor !== '0');
        if (keys.length === 0) return [];
        const values = await this.redis.mget(keys);
        const list: Array<{ id: string; data: LoginSession }> = [];
        keys.forEach((key, idx) => {
            const raw = values[idx];
            if (!raw) return;
            try {
                const data = JSON.parse(raw) as LoginSession;
                const id = key.replace('fn_knock:session:', '');
                list.push({ id, data });
            } catch {}
        });
        return list.sort((a, b) => {
            const at = Date.parse(a.data.loginTime) || 0;
            const bt = Date.parse(b.data.loginTime) || 0;
            return bt - at;
        });
    }

    async getPasskeys(): Promise<PasskeyCredential[]> {
        const raw = await this.redis.get(this.passkeyListKey);
        if (!raw) return [];
        try {
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed)) return parsed as PasskeyCredential[];
        } catch {
            return [];
        }
        return [];
    }

    async savePasskeys(passkeys: PasskeyCredential[]): Promise<void> {
        await this.redis.set(this.passkeyListKey, JSON.stringify(passkeys));
    }

    async addPasskey(passkey: PasskeyCredential): Promise<void> {
        const passkeys = await this.getPasskeys();
        passkeys.push(passkey);
        await this.savePasskeys(passkeys);
    }

    async deletePasskey(id: string): Promise<boolean> {
        const passkeys = await this.getPasskeys();
        const updated = passkeys.filter(passkey => passkey.id !== id);
        if (updated.length === passkeys.length) return false;
        await this.savePasskeys(updated);
        return true;
    }

    async updatePasskeyCounter(id: string, counter: number, lastUsedAt: string): Promise<boolean> {
        const passkeys = await this.getPasskeys();
        const target = passkeys.find(passkey => passkey.id === id);
        if (!target) return false;
        target.counter = counter;
        target.lastUsedAt = lastUsedAt;
        await this.savePasskeys(passkeys);
        return true;
    }

    async setPasskeyChallenge(challenge: string, type: 'register' | 'auth', ttlSeconds: number = 300): Promise<void> {
        await this.redis.set(`${this.passkeyChallengeKey}:${challenge}`, type, 'EX', ttlSeconds);
    }

    async consumePasskeyChallenge(challenge: string, type: 'register' | 'auth'): Promise<boolean> {
        const key = `${this.passkeyChallengeKey}:${challenge}`;
        const value = await this.redis.get(key);
        if (value !== type) return false;
        await this.redis.del(key);
        return true;
    }

    async createPasskeyBindToken(totpId: string, ttlSeconds: number = 600): Promise<string> {
        const token = randomBytes(24).toString('hex');
        await this.redis.set(`${this.passkeyBindKey}:${token}`, totpId, 'EX', ttlSeconds);
        return token;
    }

    async isPasskeyBindTokenValid(token: string): Promise<boolean> {
        const value = await this.redis.get(`${this.passkeyBindKey}:${token}`);
        return value !== null;
    }

    async consumePasskeyBindToken(token: string): Promise<string | null> {
        const key = `${this.passkeyBindKey}:${token}`;
        const value = await this.redis.get(key);
        if (!value) return null;
        await this.redis.del(key);
        return value;
    }
}

export const configManager = new ConfigManager();
