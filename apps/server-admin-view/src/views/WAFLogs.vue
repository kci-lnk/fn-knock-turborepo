<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import type { WAFEvent } from "../types";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import { normalizeIpKey } from "../composables/useIpLocationBatch";
import { useWafLogIpSelection } from "./waf-logs/useWafLogIpSelection";
import { useWafLogDisplay } from "./waf-logs/useWafLogDisplay";
import {
  getWafEventSourceIp,
  useWafLogsResource,
} from "./waf-logs/useWafLogsResource";
import WAFLogsTable from "./waf-logs/WAFLogsTable.vue";
import WAFLogsDisabledNotice from "./waf-logs/WAFLogsDisabledNotice.vue";
import WAFLogsFilters from "./waf-logs/WAFLogsFilters.vue";
import WAFLogsHeader from "./waf-logs/WAFLogsHeader.vue";
import WAFLogsPagination from "./waf-logs/WAFLogsPagination.vue";

const router = useRouter();
const { t, locale } = useI18n();

const isDetailsOpen = ref(false);
const activeEvent = ref<WAFEvent | null>(null);
const {
  availableDates,
  canLoadNewer,
  canLoadOlder,
  currentCursor,
  cursorPageLabel,
  deleteSelectedDate,
  entries,
  getSnapshot,
  handleDateChange,
  handleLimitChange,
  handleLoadFirst,
  handleLoadNewer,
  handleLoadOlder,
  handleSearch,
  isDeleting,
  isWAFEnabled,
  limit,
  loading,
  refreshAll,
  searchQuery,
  selectedDate,
  selectedWafEntryKeys,
  shouldFloatPagination,
  traceFilter,
} = useWafLogsResource();
const viewDetails = (event: WAFEvent) => {
  activeEvent.value = event;
  isDetailsOpen.value = true;
};

const goToSettings = () => {
  router.push({ path: "/system", query: { tab: "waf" } });
};

const getEntryActionIp = (event: WAFEvent) => {
  const sourceIp = getWafEventSourceIp(event);
  return normalizeIpKey(sourceIp) || sourceIp.trim();
};

const getEntrySelectionKey = (event: WAFEvent, index: number) =>
  event.trace_id ||
  [
    currentCursor.value || "first",
    index,
    event.time || "",
    event.transaction_id || "",
    event.request_uri || event.path || "",
    getEntryActionIp(event),
  ].join("|");

const getEntryDisplayIp = (event: WAFEvent) => {
  const sourceIp = getWafEventSourceIp(event);
  return normalizeIpKey(sourceIp) || sourceIp || "-";
};

const getEntryIpSnapshot = (event: WAFEvent) =>
  getSnapshot(getWafEventSourceIp(event));

const getEntryIpLocation = (event: WAFEvent) =>
  getEntryIpSnapshot(event)?.location || "";

const getEntryIpLocationText = (event: WAFEvent) => {
  const snapshot = getEntryIpSnapshot(event);
  const location = snapshot?.location || "";
  if (location) return location;

  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return t("admin.hostActiveIps.resolving");
  }

  if (snapshot?.status === "failed") {
    return t("admin.hostActiveIps.unavailable");
  }

  return "";
};

const displayedEntries = computed(() =>
  entries.value.map((entry, index) => ({
    ...entry,
    ipLocation: getEntryIpLocation(entry),
    actionIp: getEntryActionIp(entry),
    selectionKey: getEntrySelectionKey(entry, index),
  })),
);

const {
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
} = useWafLogIpSelection({
  displayedEntries,
  selectedWafEntryKeys,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const activeEventWithIpLocation = computed(() =>
  activeEvent.value
    ? {
        ...activeEvent.value,
        ipLocation: getEntryIpLocation(activeEvent.value),
      }
    : null,
);

const {
  actionLabel,
  actionVariant,
  detailCopyText,
  detailItems,
  formatPrimaryRuleId,
  formatRuleLocationSummary,
  formatRuleSummary,
  modeLabel,
  routeTypeLabel,
} = useWafLogDisplay({
  activeEvent,
  activeEventWithIpLocation,
  locale,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
</script>

<template>
  <div class="flex h-full flex-col gap-3">
    <WAFLogsHeader
      :is-blocking-ips="isBlockingIps"
      :is-deleting="isDeleting"
      :is-mutating-blacklist-ips="isMutatingBlacklistIps"
      :is-releasing-ips="isReleasingIps"
      :loading="loading"
      :selected-blocked-count="selectedBlockedWafIps.length"
      :selected-date="selectedDate"
      :selected-unblocked-count="selectedUnblockedWafIps.length"
      @block-selected="blockIpsFromWafLogs(selectedUnblockedWafIps)"
      @delete-date="deleteSelectedDate"
      @refresh="refreshAll"
      @release-selected="releaseIpsFromWafLogs(selectedBlockedWafIps)"
    />

    <WAFLogsDisabledNotice
      v-if="!isWAFEnabled"
      @open-settings="goToSettings"
    />

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <WAFLogsFilters
        v-model:search-query="searchQuery"
        :available-dates="availableDates"
        :cursor-page-label="cursorPageLabel"
        :entry-count="entries.length"
        :selected-date="selectedDate"
        :trace-filter="traceFilter"
        @date-change="handleDateChange"
        @search="handleSearch"
      />

      <WAFLogsTable
        v-model:is-all-displayed-rows-selected="isAllDisplayedRowsSelected"
        :action-label="actionLabel"
        :action-variant="actionVariant"
        :block-ips-from-waf-logs="blockIpsFromWafLogs"
        :entries="displayedEntries"
        :format-primary-rule-id="formatPrimaryRuleId"
        :format-rule-location-summary="formatRuleLocationSummary"
        :format-rule-summary="formatRuleSummary"
        :get-entry-display-ip="getEntryDisplayIp"
        :get-entry-ip-location-text="getEntryIpLocationText"
        :has-selectable-displayed-rows="hasSelectableDisplayedRows"
        :is-general-blacklisted="isGeneralBlacklisted"
        :is-mutating-blacklist-ips="isMutatingBlacklistIps"
        :loading="loading"
        :mode-label="modeLabel"
        :release-ips-from-waf-logs="releaseIpsFromWafLogs"
        :route-type-label="routeTypeLabel"
        :selected-waf-entry-keys="selectedWafEntryKeys"
        :toggle-waf-entry-selection="toggleWafEntrySelection"
        :view-details="viewDetails"
      />

      <WAFLogsPagination
        :can-load-newer="canLoadNewer"
        :can-load-older="canLoadOlder"
        :cursor-page-label="cursorPageLabel"
        :handle-limit-change="handleLimitChange"
        :handle-load-first="handleLoadFirst"
        :handle-load-newer="handleLoadNewer"
        :handle-load-older="handleLoadOlder"
        :limit="limit"
        :loading="loading"
        :should-float="shouldFloatPagination"
      />
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      :title="t('admin.wafLogs.detailTitle')"
      :description="t('admin.wafLogs.detailDescription')"
      max-width-class="sm:max-w-[680px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEvent" class="space-y-4">
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>
