const RELOAD_QUERY_KEY = "_fn_knock_reload";
const RELOAD_REASON_QUERY_KEY = "_fn_knock_reload_reason";
const CHUNK_RELOAD_STORAGE_KEY = "fn-knock:chunk-reload-at";

export const CHUNK_RELOAD_GUARD_MS = 60_000;

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

export const isDynamicImportFailure = (
  error: unknown,
  allowGenericFetchError = true,
) => {
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
      "unable to preload css for ",
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
    allowGenericFetchError &&
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
