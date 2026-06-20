import { computed, watch, type ComputedRef, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { GeneralBlacklistAPI } from "@/lib/api";
import type { WAFEvent } from "@/types";
import { useGeneralBlacklistStatus } from "@/composables/useGeneralBlacklistStatus";

type TranslateParams = Record<string, unknown>;

export type SelectableWafLogEntry = WAFEvent & {
  actionIp: string;
  selectionKey: string;
};

export const useWafLogIpSelection = ({
  displayedEntries,
  selectedWafEntryKeys,
  translate,
}: {
  displayedEntries: ComputedRef<SelectableWafLogEntry[]>;
  selectedWafEntryKeys: Ref<Set<string>>;
  translate: (key: string, params?: TranslateParams) => string;
}) => {
  const { isPending: isBlockingIps, run: runBlockIps } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.wafLogs.blacklistFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.wafLogs.blacklistFailed"),
        ),
      });
    },
  });
  const { isPending: isReleasingIps, run: runReleaseIps } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.wafLogs.unblacklistFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.wafLogs.unblacklistFailed"),
        ),
      });
    },
  });
  const isMutatingBlacklistIps = computed(
    () => isBlockingIps.value || isReleasingIps.value,
  );

  const displayedEntryKeys = computed(() =>
    displayedEntries.value.map((entry) => entry.selectionKey),
  );
  const displayedSelectableEntryKeys = computed(() =>
    displayedEntries.value
      .filter((entry) => entry.actionIp)
      .map((entry) => entry.selectionKey),
  );
  const hasSelectableDisplayedRows = computed(
    () => displayedSelectableEntryKeys.value.length > 0,
  );

  const displayedEntryIps = computed(() =>
    Array.from(
      new Set(
        displayedEntries.value
          .map((entry) => entry.actionIp)
          .filter(Boolean),
      ),
    ),
  );
  const {
    refresh: refreshGeneralBlacklistStatus,
    isBlacklisted: isGeneralBlacklisted,
  } = useGeneralBlacklistStatus(displayedEntryIps);

  const selectedWafIpList = computed(() =>
    Array.from(
      new Set(
        displayedEntries.value
          .filter((entry) =>
            selectedWafEntryKeys.value.has(entry.selectionKey),
          )
          .map((entry) => entry.actionIp)
          .filter(Boolean),
      ),
    ),
  );
  const selectedBlockedWafIps = computed(() =>
    selectedWafIpList.value.filter((ip) => isGeneralBlacklisted(ip)),
  );
  const selectedUnblockedWafIps = computed(() =>
    selectedWafIpList.value.filter((ip) => !isGeneralBlacklisted(ip)),
  );

  const isAllDisplayedRowsSelected = computed({
    get: () =>
      displayedSelectableEntryKeys.value.length > 0 &&
      displayedSelectableEntryKeys.value.every((key) =>
        selectedWafEntryKeys.value.has(key),
      ),
    set: (checked: boolean) => {
      const next = new Set(selectedWafEntryKeys.value);
      if (checked) {
        displayedEntries.value.forEach((entry) => {
          if (entry.actionIp) next.add(entry.selectionKey);
        });
      } else {
        displayedEntryKeys.value.forEach((key) => next.delete(key));
      }
      selectedWafEntryKeys.value = next;
    },
  });

  const toggleWafEntrySelection = (key?: string) => {
    if (!key) return;
    const next = new Set(selectedWafEntryKeys.value);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    selectedWafEntryKeys.value = next;
  };

  const removeSelectedWafIps = (ips: string[]) => {
    const operatedIps = new Set(ips);
    selectedWafEntryKeys.value = new Set(
      displayedEntries.value
        .filter(
          (entry) =>
            selectedWafEntryKeys.value.has(entry.selectionKey) &&
            !operatedIps.has(entry.actionIp),
        )
        .map((entry) => entry.selectionKey),
    );
  };

  const blockIpsFromWafLogs = async (ips: string[]) => {
    const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter(
      (ip) => !isGeneralBlacklisted(ip),
    );
    if (uniqueIps.length === 0) return;

    await runBlockIps(() => GeneralBlacklistAPI.add(uniqueIps, "waf_log"), {
      onSuccess: async (result) => {
        toast.success(translate("admin.wafLogs.blacklistSuccess"), {
          description: translate("admin.wafLogs.blacklistSuccessDetail", {
            added: result?.added ?? 0,
            updated: result?.updated ?? 0,
          }),
        });
        removeSelectedWafIps(uniqueIps);
        await refreshGeneralBlacklistStatus();
      },
    });
  };

  const releaseIpsFromWafLogs = async (ips: string[]) => {
    const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter((ip) =>
      isGeneralBlacklisted(ip),
    );
    if (uniqueIps.length === 0) return;

    await runReleaseIps(() => GeneralBlacklistAPI.delete(uniqueIps), {
      onSuccess: async (result) => {
        toast.success(translate("admin.wafLogs.unblacklistSuccess"), {
          description: translate("admin.wafLogs.unblacklistSuccessDetail", {
            removed: result?.removed ?? 0,
          }),
        });
        removeSelectedWafIps(uniqueIps);
        await refreshGeneralBlacklistStatus();
      },
    });
  };

  watch(displayedEntryKeys, (keys) => {
    const visibleKeys = new Set(keys);
    selectedWafEntryKeys.value = new Set(
      Array.from(selectedWafEntryKeys.value).filter((key) =>
        visibleKeys.has(key),
      ),
    );
  });

  return {
    blockIpsFromWafLogs,
    hasSelectableDisplayedRows,
    isAllDisplayedRowsSelected,
    isBlockingIps,
    isGeneralBlacklisted,
    isMutatingBlacklistIps,
    isReleasingIps,
    releaseIpsFromWafLogs,
    selectedBlockedWafIps,
    selectedUnblockedWafIps,
    toggleWafEntrySelection,
  };
};
