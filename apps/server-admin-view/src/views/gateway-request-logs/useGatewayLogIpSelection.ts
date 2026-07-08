import type { ComputedRef, Ref } from "vue";
import type { GatewayLogEntry } from "@/types";
import { useLogIpSelection } from "@/composables/useLogIpSelection";

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
  const selection = useLogIpSelection({
    displayedEntries,
    messageKeys: {
      blacklistFailed: "admin.gatewayRequestLogs.blacklistFailed",
      blacklistSuccess: "admin.gatewayRequestLogs.blacklistSuccess",
      blacklistSuccessDetail: "admin.gatewayRequestLogs.blacklistSuccessDetail",
      unblacklistFailed: "admin.gatewayRequestLogs.unblacklistFailed",
      unblacklistSuccess: "admin.gatewayRequestLogs.unblacklistSuccess",
      unblacklistSuccessDetail:
        "admin.gatewayRequestLogs.unblacklistSuccessDetail",
    },
    selectedEntryKeys: selectedLogEntryKeys,
    source: "request_log",
    translate,
  });

  return {
    blockIpsFromLogs: selection.blockIps,
    hasSelectableDisplayedRows: selection.hasSelectableDisplayedRows,
    isAllDisplayedRowsSelected: selection.isAllDisplayedRowsSelected,
    isBlockingIps: selection.isBlockingIps,
    isGeneralBlacklisted: selection.isGeneralBlacklisted,
    isMutatingBlacklistIps: selection.isMutatingBlacklistIps,
    isReleasingIps: selection.isReleasingIps,
    releaseIpsFromLogs: selection.releaseIps,
    selectedBlockedLogIps: selection.selectedBlockedIps,
    selectedUnblockedLogIps: selection.selectedUnblockedIps,
    toggleLogEntrySelection: selection.toggleEntrySelection,
  };
};
