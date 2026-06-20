import { computed, type ComputedRef, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { GeneralBlacklistAPI } from "@/lib/api";
import type { GatewayLogEntry } from "@/types";
import { useGeneralBlacklistStatus } from "@/composables/useGeneralBlacklistStatus";

type TranslateParams = Record<string, unknown>;

export type SelectableGatewayLogEntry = GatewayLogEntry & {
  actionIp: string;
  selectionKey: string;
};

export const useGatewayLogIpSelection = ({
  displayedEntries,
  selectedLogEntryKeys,
  translate,
}: {
  displayedEntries: ComputedRef<SelectableGatewayLogEntry[]>;
  selectedLogEntryKeys: Ref<Set<string>>;
  translate: (key: string, params?: TranslateParams) => string;
}) => {
  const { isPending: isBlockingIps, run: runBlockIps } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.gatewayRequestLogs.blacklistFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.gatewayRequestLogs.blacklistFailed"),
        ),
      });
    },
  });
  const { isPending: isReleasingIps, run: runReleaseIps } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.gatewayRequestLogs.unblacklistFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.gatewayRequestLogs.unblacklistFailed"),
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

  const selectedLogIpList = computed(() =>
    Array.from(
      new Set(
        displayedEntries.value
          .filter((entry) =>
            selectedLogEntryKeys.value.has(entry.selectionKey),
          )
          .map((entry) => entry.actionIp)
          .filter(Boolean),
      ),
    ),
  );
  const selectedBlockedLogIps = computed(() =>
    selectedLogIpList.value.filter((ip) => isGeneralBlacklisted(ip)),
  );
  const selectedUnblockedLogIps = computed(() =>
    selectedLogIpList.value.filter((ip) => !isGeneralBlacklisted(ip)),
  );

  const isAllDisplayedRowsSelected = computed({
    get: () =>
      displayedSelectableEntryKeys.value.length > 0 &&
      displayedSelectableEntryKeys.value.every((key) =>
        selectedLogEntryKeys.value.has(key),
      ),
    set: (checked: boolean) => {
      const next = new Set(selectedLogEntryKeys.value);
      if (checked) {
        displayedEntries.value.forEach((entry) => {
          if (entry.actionIp) next.add(entry.selectionKey);
        });
      } else {
        displayedEntryKeys.value.forEach((key) => next.delete(key));
      }
      selectedLogEntryKeys.value = next;
    },
  });

  const toggleLogEntrySelection = (key?: string) => {
    if (!key) return;
    const next = new Set(selectedLogEntryKeys.value);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    selectedLogEntryKeys.value = next;
  };

  const removeSelectedLogIps = (ips: string[]) => {
    const operatedIps = new Set(ips);
    selectedLogEntryKeys.value = new Set(
      displayedEntries.value
        .filter(
          (entry) =>
            selectedLogEntryKeys.value.has(entry.selectionKey) &&
            !operatedIps.has(entry.actionIp),
        )
        .map((entry) => entry.selectionKey),
    );
  };

  const blockIpsFromLogs = async (ips: string[]) => {
    const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter(
      (ip) => !isGeneralBlacklisted(ip),
    );
    if (uniqueIps.length === 0) return;

    await runBlockIps(
      () => GeneralBlacklistAPI.add(uniqueIps, "request_log"),
      {
        onSuccess: async (result) => {
          toast.success(translate("admin.gatewayRequestLogs.blacklistSuccess"), {
            description: translate(
              "admin.gatewayRequestLogs.blacklistSuccessDetail",
              {
                added: result?.added ?? 0,
                updated: result?.updated ?? 0,
              },
            ),
          });
          removeSelectedLogIps(uniqueIps);
          await refreshGeneralBlacklistStatus();
        },
      },
    );
  };

  const releaseIpsFromLogs = async (ips: string[]) => {
    const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter((ip) =>
      isGeneralBlacklisted(ip),
    );
    if (uniqueIps.length === 0) return;

    await runReleaseIps(() => GeneralBlacklistAPI.delete(uniqueIps), {
      onSuccess: async (result) => {
        toast.success(
          translate("admin.gatewayRequestLogs.unblacklistSuccess"),
          {
            description: translate(
              "admin.gatewayRequestLogs.unblacklistSuccessDetail",
              {
                removed: result?.removed ?? 0,
              },
            ),
          },
        );
        removeSelectedLogIps(uniqueIps);
        await refreshGeneralBlacklistStatus();
      },
    });
  };

  return {
    blockIpsFromLogs,
    hasSelectableDisplayedRows,
    isAllDisplayedRowsSelected,
    isBlockingIps,
    isGeneralBlacklisted,
    isMutatingBlacklistIps,
    isReleasingIps,
    releaseIpsFromLogs,
    selectedBlockedLogIps,
    selectedUnblockedLogIps,
    toggleLogEntrySelection,
  };
};
