import { useI18n } from "vue-i18n";
import { useLocalPagedList } from "@admin-shared/composables/useLocalPagedList";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import type { WhiteListRecord } from "@/lib/api/whitelist";
import { useWhitelistAddRecord } from "./useWhitelistAddRecord";
import { useWhitelistRecordActions } from "./useWhitelistRecordActions";
import { useWhitelistRecords } from "./useWhitelistRecords";
import {
  formatWhitelistRegionInput,
  formatWhitelistRemaining,
  getWhitelistRegionGroupLabel,
  getWhitelistResolveStatusLabel,
  getWhitelistResolveStatusVariant,
  getWhitelistTargetTypeLabel,
} from "./whitelistPresentation";

export function useIpWhitelistPage() {
  const { t } = useI18n();
  const translate = (key: string, params?: Record<string, unknown>) =>
    params ? t(key, params) : t(key);
  const { fetchRecords, isInitializing, loading, records, regionGroups } =
    useWhitelistRecords(translate);
  const showInitializingSkeleton = useDelayedLoading(isInitializing);

  const {
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    filteredItems: filteredRecords,
    pagedItems: paginatedRecords,
    handlePageChange,
    handleLimitChange,
  } = useLocalPagedList<WhiteListRecord>({
    items: records,
    normalizeQuery: (query) => query.toLowerCase(),
    filter: (record, query) =>
      record.ip.toLowerCase().includes(query) ||
      Boolean(record.comment?.toLowerCase().includes(query)) ||
      Boolean(
        record.resolvedTargets?.some((target) =>
          target.toLowerCase().includes(query),
        ),
      ),
  });

  const {
    refreshRecord,
    refreshingId,
    removeRecord,
    removeRegionGroup,
    removingId,
    removingRegionGroupId,
    saveComment,
  } = useWhitelistRecordActions({
    currentPage,
    fetchRecords,
    paginatedRecords,
    records,
    translate,
  });

  const addRecordController = useWhitelistAddRecord({
    currentPage,
    fetchRecords,
    searchQuery,
    translate,
  });

  return {
    ...addRecordController,
    currentPage,
    fetchRecords,
    filteredRecords,
    formatRegionInput: formatWhitelistRegionInput,
    formatRemaining: (expireAt: number) =>
      formatWhitelistRemaining(expireAt, translate),
    getResolveStatusLabel: (record: WhiteListRecord) =>
      getWhitelistResolveStatusLabel(record, translate),
    getResolveStatusVariant: getWhitelistResolveStatusVariant,
    handleLimitChange,
    handlePageChange,
    isInitializing,
    limit,
    loading,
    paginatedRecords,
    parsedLimit,
    records,
    refreshingId,
    refreshRecord,
    regionGroupLabel: getWhitelistRegionGroupLabel,
    regionGroups,
    removeRecord,
    removeRegionGroup,
    removingId,
    removingRegionGroupId,
    saveComment,
    searchQuery,
    showInitializingSkeleton,
    targetTypeBadgeLabel: getWhitelistTargetTypeLabel,
  };
}

export type IpWhitelistPageController = ReturnType<typeof useIpWhitelistPage>;
