import {
  computed,
  onBeforeUnmount,
  ref,
  unref,
  watch,
  type Ref,
} from "vue";
import { DashboardAPI } from "../lib/api";
import type {
  HostActiveIp,
  IpLocationLookupStatus,
  IpLocationSnapshot,
} from "../types";
import { browserT } from "@fn-knock/i18n/vue";
import { useIpLocationBatch } from "./useIpLocationBatch";

type HostSource = string | Ref<string> | (() => string);

export type HostActiveIpDisplayItem = HostActiveIp & {
  locationText: string;
  locationStatus: IpLocationLookupStatus | null;
  locationSnapshot: IpLocationSnapshot | null;
};

const DEFAULT_POLL_INTERVAL_MS = 5000;

const readHost = (source: HostSource) => {
  if (typeof source === "function") {
    return source().trim();
  }
  return unref(source).trim();
};

const getLocationText = (snapshot: IpLocationSnapshot | null) => {
  if (snapshot?.location) return snapshot.location;

  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return browserT("admin.hostActiveIps.resolving");
  }

  if (snapshot?.status === "skipped") {
    return browserT("admin.hostActiveIps.privateAddress");
  }

  if (snapshot?.status === "failed") {
    return browserT("admin.hostActiveIps.unavailable");
  }

  return browserT("admin.hostActiveIps.unavailable");
};

const normalizeWindowSeconds = (value: unknown) => {
  const seconds = Number(value ?? 120);
  if (!Number.isFinite(seconds)) return 120;
  return Math.max(1, Math.floor(seconds));
};

export const useHostActiveIps = (
  host: HostSource,
  open: Ref<boolean>,
  options: { pollIntervalMs?: number } = {},
) => {
  const items = ref<HostActiveIp[]>([]);
  const loading = ref(false);
  const error = ref("");
  const updatedAt = ref<number | null>(null);
  const windowSeconds = ref(120);
  const { trackIps, getSnapshot } = useIpLocationBatch();

  let requestId = 0;
  let pollTimer: number | null = null;

  const pollIntervalMs = Math.max(
    1000,
    options.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS,
  );

  const clearPollTimer = () => {
    if (pollTimer !== null) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const schedulePoll = () => {
    clearPollTimer();
    if (!open.value || typeof window === "undefined") return;

    pollTimer = window.setTimeout(() => {
      void load({ silent: true });
    }, pollIntervalMs);
  };

  const load = async (loadOptions: { silent?: boolean } = {}) => {
    const currentHost = readHost(host);
    const currentRequestId = ++requestId;
    clearPollTimer();

    if (!currentHost) {
      items.value = [];
      trackIps([]);
      error.value = "";
      loading.value = false;
      updatedAt.value = null;
      return;
    }

    if (!loadOptions.silent) {
      loading.value = true;
    }
    error.value = "";

    try {
      const result = await DashboardAPI.getHostActiveIps(currentHost);
      if (currentRequestId !== requestId) return;

      items.value = result.items ?? [];
      windowSeconds.value = normalizeWindowSeconds(result.window_seconds);
      updatedAt.value = result.timestamp ?? Date.now();
      trackIps(items.value.map((item) => item.ip));
    } catch (caught: any) {
      if (currentRequestId !== requestId) return;
      error.value =
        caught?.response?.data?.message ||
        caught?.message ||
        browserT("admin.hostActiveIps.loadFailed");
    } finally {
      if (currentRequestId === requestId) {
        loading.value = false;
        schedulePoll();
      }
    }
  };

  const displayItems = computed<HostActiveIpDisplayItem[]>(() =>
    items.value.map((item) => {
      const snapshot = getSnapshot(item.ip);
      return {
        ...item,
        locationText: getLocationText(snapshot),
        locationStatus: snapshot?.status ?? null,
        locationSnapshot: snapshot,
      };
    }),
  );

  watch(
    [open, () => readHost(host)],
    ([isOpen]) => {
      requestId += 1;
      clearPollTimer();
      if (!isOpen) return;
      void load();
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    requestId += 1;
    clearPollTimer();
  });

  return {
    items,
    displayItems,
    loading,
    error,
    updatedAt,
    windowSeconds,
    refresh: load,
  };
};
