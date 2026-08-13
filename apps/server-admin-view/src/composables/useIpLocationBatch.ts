import { onBeforeUnmount, ref } from "vue";
import { IpLocationAPI } from "@/lib/api/gateway";
import type { IpLocationLookupStatus, IpLocationSnapshot } from "../types";

const POLL_INTERVAL_MS = 2000;
const MAX_POLL_FAILURES = 3;

const isPendingStatus = (status?: IpLocationLookupStatus) =>
  status === "queued" || status === "processing";

export const normalizeIpKey = (ip: string) => {
  let candidate = String(ip || "").trim();
  if (!candidate) return "";

  const bracketedMatch = candidate.match(/^\[(.+)\](?::\d+)?$/);
  if (bracketedMatch?.[1]) {
    candidate = bracketedMatch[1];
  }

  const ipv4WithPortMatch = candidate.match(/^(\d+\.\d+\.\d+\.\d+):\d+$/);
  if (ipv4WithPortMatch?.[1]) {
    candidate = ipv4WithPortMatch[1];
  }

  if (candidate.includes("%")) {
    candidate = candidate.split("%")[0] || candidate;
  }

  if (candidate.startsWith("::ffff:")) {
    candidate = candidate.slice("::ffff:".length);
  }

  if (candidate === "::1") {
    candidate = "127.0.0.1";
  }

  return candidate;
};

const dedupeIps = (ips: string[]) => {
  const uniqueIps = new Map<string, string>();

  for (const ip of ips) {
    const rawIp = String(ip || "").trim();
    if (!rawIp) continue;

    const normalizedIp = normalizeIpKey(rawIp);
    const dedupeKey = normalizedIp || rawIp;
    if (!uniqueIps.has(dedupeKey)) {
      uniqueIps.set(dedupeKey, rawIp);
    }
  }

  return [...uniqueIps.values()];
};

export const useIpLocationBatch = () => {
  const snapshots = ref<Record<string, IpLocationSnapshot>>({});

  let trackedIps: string[] = [];
  let pollTimer: ReturnType<typeof window.setTimeout> | null = null;
  let activeRunId = 0;
  let consecutiveFailureCount = 0;

  const clearPollTimer = () => {
    if (pollTimer) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const mergeSnapshots = (items: IpLocationSnapshot[]) => {
    if (items.length === 0) return;

    const nextSnapshots = { ...snapshots.value };
    for (const item of items) {
      if (item.ip) {
        nextSnapshots[item.ip] = item;
      }
      if (item.normalizedIp) {
        nextSnapshots[item.normalizedIp] = item;
      }
    }
    snapshots.value = nextSnapshots;
  };

  const markSnapshotsFailed = (ips: string[], error: string) => {
    if (ips.length === 0) return;

    const nextSnapshots = { ...snapshots.value };
    const updatedAt = Date.now();
    for (const ip of ips) {
      const key = String(ip || "").trim();
      if (!key) continue;

      const current = getSnapshot(key);
      if (current && !isPendingStatus(current.status)) continue;

      const normalizedIp = normalizeIpKey(key);
      const failedSnapshot: IpLocationSnapshot = {
        ip: key,
        normalizedIp,
        status: "failed",
        attempts: current?.attempts ?? 0,
        maxAttempts: current?.maxAttempts ?? MAX_POLL_FAILURES,
        location: "",
        error,
        updatedAt,
      };

      nextSnapshots[key] = failedSnapshot;
      if (normalizedIp) {
        nextSnapshots[normalizedIp] = failedSnapshot;
      }
    }

    snapshots.value = nextSnapshots;
  };

  const scheduleNextPoll = (runId: number) => {
    clearPollTimer();

    const unresolvedIps = trackedIps.filter((ip) => {
      const snapshot = getSnapshot(ip);
      return !snapshot || isPendingStatus(snapshot.status);
    });

    if (unresolvedIps.length === 0) {
      return;
    }

    pollTimer = window.setTimeout(() => {
      if (runId !== activeRunId) return;
      void fetchBatch(unresolvedIps, runId);
    }, POLL_INTERVAL_MS);
  };

  const fetchBatch = async (ips: string[], runId: number) => {
    const uniqueIps = dedupeIps(ips);
    if (uniqueIps.length === 0) {
      clearPollTimer();
      return;
    }

    try {
      const items = await IpLocationAPI.lookupBatch(uniqueIps);
      if (runId !== activeRunId) return;
      consecutiveFailureCount = 0;
      mergeSnapshots(items);
    } catch (error) {
      if (runId !== activeRunId) return;
      consecutiveFailureCount += 1;
      console.error("[ip-location] failed to fetch batch:", error);

      if (consecutiveFailureCount >= MAX_POLL_FAILURES) {
        markSnapshotsFailed(
          uniqueIps,
          `batch lookup failed after ${MAX_POLL_FAILURES} attempts`,
        );
        clearPollTimer();
        return;
      }
    }

    if (runId !== activeRunId) return;
    scheduleNextPoll(runId);
  };

  const trackIps = (ips: string[]) => {
    trackedIps = dedupeIps(ips);
    activeRunId += 1;
    consecutiveFailureCount = 0;
    const runId = activeRunId;

    clearPollTimer();
    if (trackedIps.length === 0) {
      return;
    }

    void fetchBatch(trackedIps, runId);
  };

  const getSnapshot = (ip?: string | null) => {
    const key = String(ip || "").trim();
    if (!key) return null;
    return snapshots.value[key] || snapshots.value[normalizeIpKey(key)] || null;
  };

  onBeforeUnmount(() => {
    clearPollTimer();
  });

  return {
    snapshots,
    trackIps,
    getSnapshot,
  };
};
