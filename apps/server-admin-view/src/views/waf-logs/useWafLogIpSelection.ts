import type { ComputedRef, Ref } from "vue";
import type { WAFEvent } from "@/types";
import { useLogIpSelection } from "@/composables/useLogIpSelection";

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
  const selection = useLogIpSelection({
    displayedEntries,
    messageKeys: {
      blacklistFailed: "admin.wafLogs.blacklistFailed",
      blacklistSuccess: "admin.wafLogs.blacklistSuccess",
      blacklistSuccessDetail: "admin.wafLogs.blacklistSuccessDetail",
      unblacklistFailed: "admin.wafLogs.unblacklistFailed",
      unblacklistSuccess: "admin.wafLogs.unblacklistSuccess",
      unblacklistSuccessDetail: "admin.wafLogs.unblacklistSuccessDetail",
    },
    pruneInvisibleSelection: true,
    selectedEntryKeys: selectedWafEntryKeys,
    source: "waf_log",
    translate,
  });

  return {
    blockIpsFromWafLogs: selection.blockIps,
    hasSelectableDisplayedRows: selection.hasSelectableDisplayedRows,
    isAllDisplayedRowsSelected: selection.isAllDisplayedRowsSelected,
    isBlockingIps: selection.isBlockingIps,
    isGeneralBlacklisted: selection.isGeneralBlacklisted,
    isMutatingBlacklistIps: selection.isMutatingBlacklistIps,
    isReleasingIps: selection.isReleasingIps,
    releaseIpsFromWafLogs: selection.releaseIps,
    selectedBlockedWafIps: selection.selectedBlockedIps,
    selectedUnblockedWafIps: selection.selectedUnblockedIps,
    toggleWafEntrySelection: selection.toggleEntrySelection,
  };
};
