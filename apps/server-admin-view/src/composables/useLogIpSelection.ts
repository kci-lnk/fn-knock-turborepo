import { computed, watch, type ComputedRef, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  GeneralBlacklistAPI,
  type GeneralBlacklistSource,
} from "@/lib/api/security";
import { useGeneralBlacklistStatus } from "@/composables/useGeneralBlacklistStatus";

type TranslateParams = Record<string, unknown>;
type Translate = (key: string, params?: TranslateParams) => string;

type SelectableLogIpEntry = {
  actionIp: string;
  selectionKey: string;
};

type LogIpSelectionMessageKeys = {
  blacklistFailed: string;
  blacklistSuccess: string;
  blacklistSuccessDetail: string;
  unblacklistFailed: string;
  unblacklistSuccess: string;
  unblacklistSuccessDetail: string;
};

export const useLogIpSelection = <Entry extends SelectableLogIpEntry>({
  displayedEntries,
  messageKeys,
  onMutated,
  pruneInvisibleSelection = false,
  selectedEntryKeys,
  source,
  translate,
}: {
  displayedEntries: ComputedRef<Entry[]>;
  messageKeys: LogIpSelectionMessageKeys;
  onMutated?: () => void | Promise<void>;
  pruneInvisibleSelection?: boolean;
  selectedEntryKeys: Ref<Set<string>>;
  source: GeneralBlacklistSource;
  translate: Translate;
}) => {
  const { isPending: isBlockingIps, run: runBlockIps } = useAsyncAction({
    onError: (error) => {
      toast.error(translate(messageKeys.blacklistFailed), {
        description: extractErrorMessage(
          error,
          translate(messageKeys.blacklistFailed),
        ),
      });
    },
  });
  const { isPending: isReleasingIps, run: runReleaseIps } = useAsyncAction({
    onError: (error) => {
      toast.error(translate(messageKeys.unblacklistFailed), {
        description: extractErrorMessage(
          error,
          translate(messageKeys.unblacklistFailed),
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
        displayedEntries.value.map((entry) => entry.actionIp).filter(Boolean),
      ),
    ),
  );

  const {
    refresh: refreshGeneralBlacklistStatus,
    isBlacklisted: isGeneralBlacklisted,
  } = useGeneralBlacklistStatus(displayedEntryIps);

  const selectedIpList = computed(() =>
    Array.from(
      new Set(
        displayedEntries.value
          .filter((entry) => selectedEntryKeys.value.has(entry.selectionKey))
          .map((entry) => entry.actionIp)
          .filter(Boolean),
      ),
    ),
  );
  const selectedBlockedIps = computed(() =>
    selectedIpList.value.filter((ip) => isGeneralBlacklisted(ip)),
  );
  const selectedUnblockedIps = computed(() =>
    selectedIpList.value.filter((ip) => !isGeneralBlacklisted(ip)),
  );

  const isAllDisplayedRowsSelected = computed({
    get: () =>
      displayedSelectableEntryKeys.value.length > 0 &&
      displayedSelectableEntryKeys.value.every((key) =>
        selectedEntryKeys.value.has(key),
      ),
    set: (checked: boolean) => {
      const next = new Set(selectedEntryKeys.value);
      if (checked) {
        displayedEntries.value.forEach((entry) => {
          if (entry.actionIp) next.add(entry.selectionKey);
        });
      } else {
        displayedEntryKeys.value.forEach((key) => next.delete(key));
      }
      selectedEntryKeys.value = next;
    },
  });

  const toggleEntrySelection = (key?: string) => {
    if (!key) return;
    const next = new Set(selectedEntryKeys.value);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    selectedEntryKeys.value = next;
  };

  const removeSelectedIps = (ips: string[]) => {
    const operatedIps = new Set(ips);
    selectedEntryKeys.value = new Set(
      displayedEntries.value
        .filter(
          (entry) =>
            selectedEntryKeys.value.has(entry.selectionKey) &&
            !operatedIps.has(entry.actionIp),
        )
        .map((entry) => entry.selectionKey),
    );
  };

  const blockIps = async (ips: string[]) => {
    const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter(
      (ip) => !isGeneralBlacklisted(ip),
    );
    if (uniqueIps.length === 0) return;

    await runBlockIps(() => GeneralBlacklistAPI.add(uniqueIps, source), {
      onSuccess: async (result) => {
        toast.success(translate(messageKeys.blacklistSuccess), {
          description: translate(messageKeys.blacklistSuccessDetail, {
            added: result?.added ?? 0,
            updated: result?.updated ?? 0,
          }),
        });
        removeSelectedIps(uniqueIps);
        await refreshGeneralBlacklistStatus();
        await onMutated?.();
      },
    });
  };

  const releaseIps = async (ips: string[]) => {
    const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter((ip) =>
      isGeneralBlacklisted(ip),
    );
    if (uniqueIps.length === 0) return;

    await runReleaseIps(() => GeneralBlacklistAPI.delete(uniqueIps), {
      onSuccess: async (result) => {
        toast.success(translate(messageKeys.unblacklistSuccess), {
          description: translate(messageKeys.unblacklistSuccessDetail, {
            removed: result?.removed ?? 0,
          }),
        });
        removeSelectedIps(uniqueIps);
        await refreshGeneralBlacklistStatus();
        await onMutated?.();
      },
    });
  };

  if (pruneInvisibleSelection) {
    watch(displayedEntryKeys, (keys) => {
      const visibleKeys = new Set(keys);
      selectedEntryKeys.value = new Set(
        Array.from(selectedEntryKeys.value).filter((key) =>
          visibleKeys.has(key),
        ),
      );
    });
  }

  return {
    blockIps,
    hasSelectableDisplayedRows,
    isAllDisplayedRowsSelected,
    isBlockingIps,
    isGeneralBlacklisted,
    isMutatingBlacklistIps,
    isReleasingIps,
    releaseIps,
    selectedBlockedIps,
    selectedUnblockedIps,
    toggleEntrySelection,
  };
};
