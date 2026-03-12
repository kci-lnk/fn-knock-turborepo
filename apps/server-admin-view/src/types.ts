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

export interface AppConfig {
    run_type: 0 | 1;
    whitelist_ips: string[];
    default_route: string;
    proxy_mappings: ProxyMapping[];
    default_tunnel?: 'frp' | 'cloudflared';
    ssl: { enabled: boolean };
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

export type SessionRecord = LoginSession & { id: string };

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
