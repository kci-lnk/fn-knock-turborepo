export const UPDATE_READY_TIMEOUT_MS = 5 * 60_000;
export const UPDATE_READY_POLL_MS = 1_000;

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
