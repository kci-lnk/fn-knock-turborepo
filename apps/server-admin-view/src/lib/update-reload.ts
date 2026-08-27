const RELOAD_QUERY_KEY = "_fn_knock_reload";
const RELOAD_REASON_QUERY_KEY = "_fn_knock_reload_reason";
const CHUNK_RELOAD_STORAGE_KEY = "fn-knock:chunk-reload-at";

export const UPDATE_READY_TIMEOUT_MS = 5 * 60_000;
export const UPDATE_READY_POLL_MS = 1_000;
export const CHUNK_RELOAD_GUARD_MS = 60_000;

type UpdateVersionStatus = {
  localVersion?: string | null;
};

type WaitForUpdatedApplicationOptions<T extends UpdateVersionStatus> = {
  loadStatus: () => Promise<T>;
  targetVersion?: string | null;
  previousVersion?: string | null;
  timeoutMs?: number;
  intervalMs?: number;
  now?: () => number;
  sleep?: (delayMs: number) => Promise<void>;
};

const normalizeVersion = (version?: string | null) =>
  version?.trim().replace(/^v(?=\d)/iu, "") ?? "";

export const isUpdatedApplicationReady = (
  status: UpdateVersionStatus,
  targetVersion?: string | null,
  previousVersion?: string | null,
) => {
  const current = normalizeVersion(status.localVersion);
  const target = normalizeVersion(targetVersion);
  if (!current) return false;
  if (target) return current === target;

  const previous = normalizeVersion(previousVersion);
  return Boolean(previous) && current !== previous;
};

const defaultSleep = (delayMs: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, delayMs));

const monotonicNow = () =>
  typeof performance === "undefined" ? Date.now() : performance.now();

export async function waitForUpdatedApplication<T extends UpdateVersionStatus>({
  loadStatus,
  targetVersion,
  previousVersion,
  timeoutMs = UPDATE_READY_TIMEOUT_MS,
  intervalMs = UPDATE_READY_POLL_MS,
  now = monotonicNow,
  sleep = defaultSleep,
}: WaitForUpdatedApplicationOptions<T>): Promise<T | null> {
  const startedAt = now();

  while (true) {
    try {
      const status = await loadStatus();
      if (isUpdatedApplicationReady(status, targetVersion, previousVersion)) {
        return status;
      }
    } catch {
      // The CGI returns 502 while fnOS replaces and restarts the FPK. Keep
      // waiting until the new backend reports its own version.
    }

    const remainingMs = timeoutMs - (now() - startedAt);
    if (remainingMs <= 0) return null;
    await sleep(Math.min(intervalMs, remainingMs));
  }
}

export const buildCacheBustedApplicationUrl = (
  href: string,
  timestamp = Date.now(),
  reason: "update" | "chunk" = "update",
) => {
  const url = new URL(href);
  url.searchParams.set(RELOAD_QUERY_KEY, String(timestamp));
  url.searchParams.set(RELOAD_REASON_QUERY_KEY, reason);
  return url.toString();
};

export const replaceWithUpdatedApplication = (
  reason: "update" | "chunk" = "update",
  timestamp = Date.now(),
) => {
  window.location.replace(
    buildCacheBustedApplicationUrl(window.location.href, timestamp, reason),
  );
};

export const isDynamicImportFailure = (error: unknown) => {
  const message = error instanceof Error ? error.message : String(error ?? "");
  const name = error instanceof Error ? error.name : "";
  const description = `${name} ${message}`.toLowerCase();
  if (
    [
      "failed to fetch dynamically imported module",
      "importing a module script failed",
      "error loading dynamically imported module",
      "load failed for module with source",
      "chunkloaderror",
    ].some((fragment) => description.includes(fragment))
  ) {
    return true;
  }

  // Chromium-based WebViews and older Safari builds can report a failed
  // dynamic import as only a generic TypeError. Router errors and bootstrap
  // errors reach this helper only while resolving an async module, so these
  // otherwise ambiguous messages are safe to treat as chunk failures here.
  const normalizedMessage = message.trim().toLowerCase();
  return (
    name.toLowerCase() === "typeerror" &&
    (normalizedMessage === "failed to fetch" ||
      normalizedMessage === "load failed")
  );
};

const recentTimestamp = (
  value: string | null,
  now: number,
  guardMs: number,
) => {
  if (!value) return false;
  const timestamp = Number(value);
  return Number.isFinite(timestamp) && Math.abs(now - timestamp) < guardMs;
};

export const claimChunkReload = (
  href: string,
  storage: Pick<Storage, "getItem" | "setItem"> | null,
  now = Date.now(),
  guardMs = CHUNK_RELOAD_GUARD_MS,
) => {
  const url = new URL(href);
  if (
    url.searchParams.get(RELOAD_REASON_QUERY_KEY) === "chunk" &&
    recentTimestamp(url.searchParams.get(RELOAD_QUERY_KEY), now, guardMs)
  ) {
    return false;
  }

  try {
    if (
      storage &&
      recentTimestamp(storage.getItem(CHUNK_RELOAD_STORAGE_KEY), now, guardMs)
    ) {
      return false;
    }
    storage?.setItem(CHUNK_RELOAD_STORAGE_KEY, String(now));
  } catch {
    // The URL timestamp still prevents a reload loop when storage is blocked.
  }
  return true;
};
