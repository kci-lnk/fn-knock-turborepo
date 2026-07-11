export const AUTH_AUTO_REDIRECT_GUARD_WINDOW_MS = 30_000;
export const AUTH_AUTO_REDIRECT_GUARD_MAX_REDIRECTS = 3;

const AUTH_AUTO_REDIRECT_GUARD_STORAGE_KEY =
  "server-auth-view:auto-redirect-guard";

const CACHE_NOISE_QUERY_PARAMS = new Set([
  "_",
  "_t",
  "_ts",
  "cache_bust",
  "cacheBust",
  "cachebuster",
]);

export interface RedirectGuardStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export type AuthRedirectBlockReason =
  | "invalid_redirect"
  | "self_redirect"
  | "repeat_redirect";

export type AuthRedirectDecision =
  | {
      allowed: true;
      redirectUrl: string;
      targetKey: string;
    }
  | {
      allowed: false;
      reason: AuthRedirectBlockReason;
    };

interface AutoRedirectRecord {
  target: string;
  redirectedAt: number;
  windowStartedAt: number;
  redirectCount: number;
}

interface GuardAutoRedirectOptions {
  redirectTo: string;
  currentUrl: string;
  storage?: RedirectGuardStorage | null;
  now?: number;
  windowMs?: number;
}

interface EffectiveLocation {
  pathname: string;
  searchParams: URLSearchParams;
  hash: string;
}

function authBasePrefix(pathname: string) {
  if (pathname === "/__auth__" || pathname.startsWith("/__auth__/")) {
    return "/__auth__";
  }
  if (pathname === "/auth" || pathname.startsWith("/auth/")) {
    return "/auth";
  }
  return "";
}

function joinAuthPath(prefix: string, pathname: string) {
  const normalizedPathname = pathname.startsWith("/")
    ? pathname
    : `/${pathname}`;
  return prefix ? `${prefix}${normalizedPathname}` : normalizedPathname;
}

/**
 * Converts the old hash-router form (/#/login) to the history-router form
 * (/login). Query parameters can live outside or inside the legacy hash, so
 * both sets are retained for redirect identity checks.
 */
function isAuthHashRouteContainer(pathname: string) {
  const normalized = normalizedPathname(pathname);
  return (
    normalized === "/" || normalized === "/auth" || normalized === "/__auth__"
  );
}

function effectiveLocation(url: URL, authOrigin: string): EffectiveLocation {
  if (
    url.origin !== authOrigin ||
    !url.hash.startsWith("#/") ||
    !isAuthHashRouteContainer(url.pathname)
  ) {
    return {
      pathname: url.pathname,
      searchParams: new URLSearchParams(url.search),
      hash: url.hash,
    };
  }

  const hashRoute = new URL(url.hash.slice(1), `${url.origin}/`);
  if (normalizedPathname(hashRoute.pathname) !== "/login") {
    return {
      pathname: url.pathname,
      searchParams: new URLSearchParams(url.search),
      hash: url.hash,
    };
  }
  const searchParams = new URLSearchParams(url.search);
  for (const [key, value] of hashRoute.searchParams) {
    searchParams.append(key, value);
  }

  return {
    pathname: joinAuthPath(authBasePrefix(url.pathname), hashRoute.pathname),
    searchParams,
    hash: "",
  };
}

function normalizedPathname(pathname: string) {
  if (pathname.length > 1 && pathname.endsWith("/")) {
    return pathname.replace(/\/+$/, "");
  }
  return pathname || "/";
}

function canonicalTarget(url: URL, authOrigin: string) {
  const effective = effectiveLocation(url, authOrigin);
  const queryEntries = [...effective.searchParams.entries()]
    .filter(([key]) => !CACHE_NOISE_QUERY_PARAMS.has(key))
    .sort(([leftKey, leftValue], [rightKey, rightValue]) => {
      const keyComparison = leftKey.localeCompare(rightKey);
      return keyComparison || leftValue.localeCompare(rightValue);
    });

  const canonical = new URL(url.origin);
  canonical.pathname = effective.pathname;
  for (const [key, value] of queryEntries) {
    canonical.searchParams.append(key, value);
  }
  canonical.hash = effective.hash;
  return canonical.href;
}

function isUnsafeRedirectReference(value: string) {
  const normalized = value.trim();
  if (
    normalized.includes("\\") ||
    [...normalized].some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint <= 0x1f || codePoint === 0x7f;
    })
  ) {
    return true;
  }
  if (normalized.length < 2) {
    return false;
  }
  return normalized[0] === "/" && normalized[1] === "/";
}

export function canonicalizeRedirectTarget(
  redirectTo: string,
  currentUrl: string,
): string | null {
  if (isUnsafeRedirectReference(redirectTo)) {
    return null;
  }
  try {
    const current = new URL(currentUrl);
    return canonicalTarget(new URL(redirectTo, current), current.origin);
  } catch {
    return null;
  }
}

export function inspectAuthRedirect(
  redirectTo: string,
  currentUrl: string,
): AuthRedirectDecision {
  let target: URL;
  let current: URL;
  if (isUnsafeRedirectReference(redirectTo)) {
    return { allowed: false, reason: "invalid_redirect" };
  }
  try {
    target = new URL(redirectTo, currentUrl);
    current = new URL(currentUrl);
  } catch {
    return { allowed: false, reason: "invalid_redirect" };
  }
  if (target.protocol !== "http:" && target.protocol !== "https:") {
    return { allowed: false, reason: "invalid_redirect" };
  }

  const targetLocation = effectiveLocation(target, current.origin);
  const currentLocation = effectiveLocation(current, current.origin);
  if (
    target.origin === current.origin &&
    normalizedPathname(targetLocation.pathname) ===
      normalizedPathname(currentLocation.pathname)
  ) {
    return { allowed: false, reason: "self_redirect" };
  }

  return {
    allowed: true,
    redirectUrl: target.href,
    targetKey: canonicalTarget(target, current.origin),
  };
}

function readAutoRedirectRecord(
  storage?: RedirectGuardStorage | null,
): AutoRedirectRecord | null {
  if (!storage) {
    return null;
  }

  try {
    const raw = storage.getItem(AUTH_AUTO_REDIRECT_GUARD_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as Partial<AutoRedirectRecord>;
    if (
      typeof parsed.target !== "string" ||
      typeof parsed.redirectedAt !== "number" ||
      !Number.isFinite(parsed.redirectedAt)
    ) {
      storage.removeItem(AUTH_AUTO_REDIRECT_GUARD_STORAGE_KEY);
      return null;
    }
    return {
      target: parsed.target,
      redirectedAt: parsed.redirectedAt,
      windowStartedAt:
        typeof parsed.windowStartedAt === "number" &&
        Number.isFinite(parsed.windowStartedAt)
          ? parsed.windowStartedAt
          : parsed.redirectedAt,
      redirectCount:
        typeof parsed.redirectCount === "number" &&
        Number.isFinite(parsed.redirectCount) &&
        parsed.redirectCount >= 1
          ? Math.floor(parsed.redirectCount)
          : 1,
    };
  } catch {
    return null;
  }
}

function writeAutoRedirectRecord(
  storage: RedirectGuardStorage | null | undefined,
  record: AutoRedirectRecord,
) {
  if (!storage) {
    return;
  }

  try {
    storage.setItem(
      AUTH_AUTO_REDIRECT_GUARD_STORAGE_KEY,
      JSON.stringify(record),
    );
  } catch {
    // Storage can be unavailable in privacy modes. Self-redirect protection
    // still works; only cross-navigation loop detection is skipped.
  }
}

/**
 * Allows the first automatic redirect and records it in sessionStorage. The
 * same target is blocked on its second attempt, while a small total redirect
 * budget also stops alternating or nonce-changing loops in the short window.
 */
export function guardAuthAutoRedirect({
  redirectTo,
  currentUrl,
  storage,
  now = Date.now(),
  windowMs = AUTH_AUTO_REDIRECT_GUARD_WINDOW_MS,
}: GuardAutoRedirectOptions): AuthRedirectDecision {
  const inspected = inspectAuthRedirect(redirectTo, currentUrl);
  if (!inspected.allowed) {
    return inspected;
  }

  const previous = readAutoRedirectRecord(storage);
  const elapsed = previous ? now - previous.redirectedAt : null;
  if (
    previous?.target === inspected.targetKey &&
    elapsed !== null &&
    elapsed >= 0 &&
    elapsed <= Math.max(0, windowMs)
  ) {
    return { allowed: false, reason: "repeat_redirect" };
  }

  const windowElapsed = previous ? now - previous.windowStartedAt : null;
  const withinWindow =
    previous !== null &&
    windowElapsed !== null &&
    windowElapsed >= 0 &&
    windowElapsed <= Math.max(0, windowMs);
  const redirectCount = withinWindow ? previous.redirectCount + 1 : 1;
  if (redirectCount > AUTH_AUTO_REDIRECT_GUARD_MAX_REDIRECTS) {
    return { allowed: false, reason: "repeat_redirect" };
  }

  writeAutoRedirectRecord(storage, {
    target: inspected.targetKey,
    redirectedAt: now,
    windowStartedAt: withinWindow ? previous.windowStartedAt : now,
    redirectCount,
  });
  return inspected;
}

export function resetAuthAutoRedirectGuard(
  storage?: RedirectGuardStorage | null,
) {
  if (!storage) {
    return;
  }

  try {
    storage.removeItem(AUTH_AUTO_REDIRECT_GUARD_STORAGE_KEY);
  } catch {
    // A successful login must not fail just because storage is unavailable.
  }
}
