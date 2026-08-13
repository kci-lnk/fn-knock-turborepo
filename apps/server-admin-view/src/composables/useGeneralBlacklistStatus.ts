import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import {
  GeneralBlacklistAPI,
  type GeneralBlacklistRecord,
} from "@/lib/api/security";
import { normalizeIpKey } from "./useIpLocationBatch";

type MaybeIpListRef = Ref<string[]> | ComputedRef<string[]>;

const normalizeTrackedIp = (ip?: string | null) => {
  const normalized = normalizeIpKey(String(ip || ""));
  return normalized || String(ip || "").trim();
};

const uniqueIps = (ips: string[]) =>
  Array.from(
    new Set(
      ips.map((ip) => normalizeTrackedIp(ip)).filter((ip) => Boolean(ip)),
    ),
  );

export const useGeneralBlacklistStatus = (ips: MaybeIpListRef) => {
  const records = ref<Record<string, GeneralBlacklistRecord>>({});
  const loading = ref(false);
  let runId = 0;

  const refresh = async (overrideIps?: string[]) => {
    const trackedIps = uniqueIps(overrideIps ?? ips.value);
    const currentRunId = ++runId;

    if (trackedIps.length === 0) {
      records.value = {};
      return;
    }

    loading.value = true;
    try {
      const data = await GeneralBlacklistAPI.getStatus(trackedIps);
      if (currentRunId !== runId) return;

      const nextRecords: Record<string, GeneralBlacklistRecord> = {};
      for (const [ip, record] of Object.entries(data.records || {})) {
        const normalizedIp = normalizeTrackedIp(ip);
        if (normalizedIp) nextRecords[normalizedIp] = record;
        if (record.ip) nextRecords[normalizeTrackedIp(record.ip)] = record;
      }
      records.value = nextRecords;
    } catch (error) {
      if (currentRunId !== runId) return;
      console.error("[general-blacklist] failed to fetch status:", error);
      const trackedSet = new Set(trackedIps);
      records.value = Object.fromEntries(
        Object.entries(records.value).filter(([ip, record]) => {
          if (trackedSet.has(ip)) return true;
          const recordIp = normalizeTrackedIp(record.ip);
          return Boolean(recordIp && trackedSet.has(recordIp));
        }),
      );
    } finally {
      if (currentRunId === runId) {
        loading.value = false;
      }
    }
  };

  const getRecord = (ip?: string | null) => {
    const normalizedIp = normalizeTrackedIp(ip);
    return normalizedIp ? records.value[normalizedIp] || null : null;
  };

  const isBlacklisted = (ip?: string | null) => Boolean(getRecord(ip));

  const splitByStatus = (candidateIps: string[]) => {
    const normalizedIps = uniqueIps(candidateIps);
    return {
      blocked: normalizedIps.filter((ip) => isBlacklisted(ip)),
      unblocked: normalizedIps.filter((ip) => !isBlacklisted(ip)),
    };
  };

  const blockedIps = computed(() =>
    uniqueIps(ips.value).filter((ip) => isBlacklisted(ip)),
  );
  const unblockedIps = computed(() =>
    uniqueIps(ips.value).filter((ip) => !isBlacklisted(ip)),
  );

  watch(
    ips,
    () => {
      void refresh();
    },
    { immediate: true },
  );

  return {
    records,
    loading,
    refresh,
    getRecord,
    isBlacklisted,
    splitByStatus,
    blockedIps,
    unblockedIps,
  };
};
