import axios from "axios";

const AUTH_BOOTSTRAP_PATH = "/__auth__/api/auth/bootstrap";
const DEFAULT_AUTH_PROBE_TIMEOUT_MS = 3_000;
const DEFAULT_NAVIGATION_CONFIRM_TIMEOUT_MS = 3_000;

type JsonRecord = Record<string, unknown>;

export interface GatewayAuthRecoveryLocation {
  href: string;
  origin: string;
  replace(url: string): void;
}

export interface GatewayAuthRecoveryOptions {
  fetchImpl: typeof fetch;
  location: GatewayAuthRecoveryLocation;
  navigationTarget?: Pick<
    Window,
    "addEventListener" | "removeEventListener"
  >;
  navigationTimeoutMs?: number;
  timeoutMs?: number;
  now?: () => number;
}

export interface GatewayAuthRecovery {
  recover(error: unknown): Promise<boolean>;
}

const isRecord = (value: unknown): value is JsonRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value);

interface NormalizedGatewayAuthRecoveryOptions {
  fetchImpl: typeof fetch;
  location: GatewayAuthRecoveryLocation;
  navigationTarget?: GatewayAuthRecoveryOptions["navigationTarget"];
  navigationTimeoutMs: number;
  timeoutMs: number;
  now: () => number;
}

export const isAxiosNetworkErrorWithoutResponse = (
  error: unknown,
): boolean => {
  if (!axios.isAxiosError(error) || error.response != null) return false;

  return (
    error.code === "ERR_NETWORK" ||
    error.message.trim().toLowerCase() === "network error"
  );
};

const readUnauthenticatedRedirect = (payload: unknown): string | null => {
  if (!isRecord(payload) || payload.success !== true) return null;
  const data = payload.data;
  if (!isRecord(data) || !isRecord(data.auth)) return null;
  if (data.auth.authenticated !== false) return null;

  return typeof data.redirect_to === "string" ? data.redirect_to.trim() : "";
};

const buildLoginRedirect = (
  redirectTo: string,
  location: GatewayAuthRecoveryLocation,
): string | null => {
  if (redirectTo) {
    try {
      const candidate = new URL(redirectTo, location.origin);
      if (
        (candidate.protocol === "http:" || candidate.protocol === "https:") &&
        candidate.username === "" &&
        candidate.password === "" &&
        candidate.href !== location.href
      ) {
        return candidate.href;
      }
    } catch {
      // Fall through to the gateway's same-origin login route.
    }
  }

  try {
    const fallback = new URL("/__auth__/login", location.origin);
    fallback.searchParams.set("redirect_uri", location.href);
    return fallback.href;
  } catch {
    return null;
  }
};

const probeGatewayAuth = async ({
  fetchImpl,
  location,
  timeoutMs,
  now,
}: NormalizedGatewayAuthRecoveryOptions): Promise<string | null> => {
  const controller = new AbortController();
  const timeout = globalThis.setTimeout(
    () => controller.abort(),
    Math.max(1, timeoutMs),
  );

  try {
    const probeUrl = new URL(AUTH_BOOTSTRAP_PATH, location.origin);
    probeUrl.searchParams.set("redirect_uri", location.href);
    probeUrl.searchParams.set("_ts", now().toString());

    const response = await fetchImpl(probeUrl, {
      method: "GET",
      credentials: "include",
      cache: "no-store",
      headers: {
        Accept: "application/json",
        "Cache-Control": "no-cache",
        Pragma: "no-cache",
      },
      signal: controller.signal,
    });
    if (!response.ok) return null;

    const mediaType = response.headers
      .get("content-type")
      ?.split(";", 1)[0]
      ?.trim()
      .toLowerCase();
    if (mediaType !== "application/json") return null;

    const redirectTo = readUnauthenticatedRedirect(await response.json());
    if (redirectTo === null) return null;
    return buildLoginRedirect(redirectTo, location);
  } catch {
    return null;
  } finally {
    globalThis.clearTimeout(timeout);
  }
};

interface NavigationConfirmation {
  result: Promise<boolean>;
  cancel(): void;
}

const beginNavigationConfirmation = (
  target: GatewayAuthRecoveryOptions["navigationTarget"],
  timeoutMs: number,
): NavigationConfirmation => {
  if (!target) {
    return {
      result: Promise.resolve(true),
      cancel() {},
    };
  }

  let finish: (confirmed: boolean) => void = () => undefined;
  const result = new Promise<boolean>((resolve) => {
    let completed = false;
    const onPageHide = () => finish(true);
    const timeout = globalThis.setTimeout(
      () => finish(false),
      Math.max(1, timeoutMs),
    );

    finish = (confirmed) => {
      if (completed) return;
      completed = true;
      globalThis.clearTimeout(timeout);
      target.removeEventListener("pagehide", onPageHide);
      resolve(confirmed);
    };
    target.addEventListener("pagehide", onPageHide, { once: true });
  });

  return {
    result,
    cancel: () => finish(false),
  };
};

export const createGatewayAuthRecovery = (
  options: GatewayAuthRecoveryOptions,
): GatewayAuthRecovery => {
  const normalizedOptions: NormalizedGatewayAuthRecoveryOptions = {
    ...options,
    navigationTimeoutMs:
      options.navigationTimeoutMs ?? DEFAULT_NAVIGATION_CONFIRM_TIMEOUT_MS,
    timeoutMs: options.timeoutMs ?? DEFAULT_AUTH_PROBE_TIMEOUT_MS,
    now: options.now ?? Date.now,
  };
  let activeProbe: Promise<boolean> | null = null;
  let redirectStarted = false;

  const probeAndRedirect = async () => {
    const redirectUrl = await probeGatewayAuth(normalizedOptions);
    if (!redirectUrl) return false;

    const confirmation = beginNavigationConfirmation(
      normalizedOptions.navigationTarget,
      normalizedOptions.navigationTimeoutMs,
    );
    try {
      redirectStarted = true;
      normalizedOptions.location.replace(redirectUrl);
      const confirmed = await confirmation.result;
      if (!confirmed) redirectStarted = false;
      return confirmed;
    } catch {
      confirmation.cancel();
      redirectStarted = false;
      return false;
    }
  };

  return {
    recover(error: unknown) {
      if (!isAxiosNetworkErrorWithoutResponse(error)) {
        return Promise.resolve(false);
      }
      if (activeProbe) return activeProbe;
      if (redirectStarted) return Promise.resolve(true);

      const probe = probeAndRedirect().finally(() => {
        if (activeProbe === probe) activeProbe = null;
      });
      activeProbe = probe;
      return probe;
    },
  };
};
