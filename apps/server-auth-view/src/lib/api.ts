import { createSignedApiClient } from '@frontend-core/api/createSignedApiClient';

const detectAppBasePrefix = () => {
    if (typeof window === 'undefined') return '';
    const pathname = window.location.pathname || '/';

    if (pathname === '/__auth__' || pathname.startsWith('/__auth__/')) {
        return '/__auth__';
    }

    if (pathname === '/auth' || pathname.startsWith('/auth/')) {
        return '/auth';
    }

    return '';
};

const joinWithBasePrefix = (basePrefix: string, path: string) => {
    const normalizedPath = path.startsWith('/') ? path : `/${path}`;
    return basePrefix ? `${basePrefix}${normalizedPath}` : normalizedPath;
};

const appBasePrefix = detectAppBasePrefix();

export const authApiBasePath = joinWithBasePrefix(appBasePrefix, '/api/auth');
export const buildAuthApiPath = (path: string) => joinWithBasePrefix(authApiBasePath, path);

const runtimeSecretPath = joinWithBasePrefix(appBasePrefix, '/__fn-knock/runtime-hmac-secret');

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
    baseURL: authApiBasePath,
    hmacSecret,
    getHmacSecret: fetchRuntimeHmacSecret,
});
