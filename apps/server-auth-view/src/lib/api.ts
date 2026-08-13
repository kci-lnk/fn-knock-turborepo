import { createApiClient } from "@frontend-core/api/createApiClient";
import type { InternalAxiosRequestConfig } from "axios";
import type {
  AuthBootstrapData,
  AuthClientLocationData,
  AuthSessionData,
} from "@frontend-core/auth/types";
import type { CaptchaPublicSettings } from "@frontend-core/captcha/types";

type NoStoreParams = Record<string, string | number | boolean | undefined>;

const detectAppBasePrefix = () => {
  if (typeof window === "undefined") return "";
  const pathname = window.location.pathname || "/";

  if (pathname === "/__auth__" || pathname.startsWith("/__auth__/")) {
    return "/__auth__";
  }

  if (pathname === "/auth" || pathname.startsWith("/auth/")) {
    return "/auth";
  }

  return "";
};

const joinWithBasePrefix = (basePrefix: string, path: string) => {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return basePrefix ? `${basePrefix}${normalizedPath}` : normalizedPath;
};

const appBasePrefix = detectAppBasePrefix();

export const authApiBasePath = joinWithBasePrefix(appBasePrefix, "/api/auth");
export const buildAuthApiPath = (path: string) =>
  joinWithBasePrefix(authApiBasePath, path);

export const withNoStoreParams = (params?: NoStoreParams) => ({
  ...(params || {}),
  _ts: Date.now(),
});

const ABSOLUTE_URL_RE = /^[a-zA-Z][a-zA-Z\d+\-.]*:/;

const buildNoStoreRequestHeaders = (headers?: HeadersInit) => {
  const next = new Headers(headers);
  next.set("Cache-Control", "no-cache");
  next.set("Pragma", "no-cache");
  return next;
};

export const withNoStoreUrl = (input: string | URL) => {
  const original = input instanceof URL ? input.toString() : input;
  const baseOrigin =
    typeof window !== "undefined" ? window.location.origin : "http://127.0.0.1";
  const url = new URL(original, baseOrigin);

  url.searchParams.set("_ts", Date.now().toString());

  if (input instanceof URL || ABSOLUTE_URL_RE.test(original)) {
    return url.toString();
  }

  return `${url.pathname}${url.search}${url.hash}`;
};

export const fetchNoStore = (input: string | URL, init?: RequestInit) =>
  fetch(withNoStoreUrl(input), {
    ...init,
    cache: "no-store",
    headers: buildNoStoreRequestHeaders(init?.headers),
  });

const applyNoStoreRequestDefaults = (config: InternalAxiosRequestConfig) => {
  const method = String(config.method || "get").toLowerCase();
  if (method !== "get" && method !== "head") {
    return config;
  }

  config.headers["Cache-Control"] = "no-cache";
  config.headers.Pragma = "no-cache";

  if (
    config.params &&
    typeof config.params === "object" &&
    !Array.isArray(config.params) &&
    "_ts" in (config.params as Record<string, unknown>)
  ) {
    return config;
  }

  config.params =
    config.params && typeof config.params === "object"
      ? {
          ...(config.params as Record<string, unknown>),
          _ts: Date.now(),
        }
      : { _ts: Date.now() };

  return config;
};

export const apiClient = createApiClient({
  baseURL: authApiBasePath,
});

apiClient.interceptors.request.use((config) =>
  applyNoStoreRequestDefaults(config),
);

export const CaptchaAPI = {
  async getConfig(): Promise<CaptchaPublicSettings> {
    const res = await apiClient.get("/captcha/config", {
      params: withNoStoreParams(),
    });
    return res.data.data;
  },
  async getPowChallenge(signal?: AbortSignal) {
    const res = await apiClient.get("/challenge", {
      params: withNoStoreParams(),
      signal,
    });
    return res.data;
  },
};

export const AuthAPI = {
  async getBootstrap(redirectUri?: string | null): Promise<AuthBootstrapData> {
    const res = await apiClient.get("/bootstrap", {
      params: withNoStoreParams(
        redirectUri ? { redirect_uri: redirectUri } : undefined,
      ),
    });
    return res.data.data;
  },
  async getSession(): Promise<AuthSessionData> {
    const res = await apiClient.get("/session", {
      params: withNoStoreParams(),
    });
    return res.data.data;
  },
  async getClientLocation(): Promise<AuthClientLocationData> {
    const res = await apiClient.get("/ip/location", {
      params: withNoStoreParams(),
    });
    return res.data.data;
  },
};
