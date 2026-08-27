<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import type { GatewayLogEntry } from "../types";
import GatewayRequestLogsActions from "./gateway-request-logs/GatewayRequestLogsActions.vue";
import GatewayRequestLogsFilters from "./gateway-request-logs/GatewayRequestLogsFilters.vue";
import GatewayRequestLogsPagination from "./gateway-request-logs/GatewayRequestLogsPagination.vue";
import GatewayRequestLogsTable from "./gateway-request-logs/GatewayRequestLogsTable.vue";
import {
  buildGatewayLogDetailCopyText,
  buildGatewayLogDetailItems,
  buildGatewayLogSelectionKey,
  getEntryActionIp,
  getEntryClientIp,
} from "./gateway-request-logs/model";
import { useGatewayLogIpSelection } from "./gateway-request-logs/useGatewayLogIpSelection";
import { useGatewayRequestLogsResource } from "./gateway-request-logs/useGatewayRequestLogsResource";
import TraceIdLink from "@/components/TraceIdLink.vue";

const router = useRouter();
const { t, locale } = useI18n();
const isDetailsOpen = ref(false);
const activeEntry = ref<GatewayLogEntry | null>(null);
const {
  activeCredentialLabel,
  activeLoggedInLabel,
  activeStatusLabel,
  activeWAFStatusLabel,
  availableDates,
  canLoadNewer,
  canLoadOlder,
  credentialFilterOptions,
  currentCursor,
  cursorPageLabel,
  deleteSelectedDate,
  entries,
  getSnapshot,
  handleCredentialChange,
  handleDateChange,
  handleLimitChange,
  handleLoadFirst,
  handleLoadNewer,
  handleLoadOlder,
  handleLoggedInChange,
  handleSearch,
  handleStatusChange,
  handleWAFStatusChange,
  isDeleting,
  limit,
  loading,
  logsDir,
  refreshAll,
  searchQuery,
  selectedCredential,
  selectedDate,
  selectedLoggedIn,
  selectedLogEntryKeys,
  selectedStatus,
  selectedWAFStatus,
  shouldFloatPagination,
  showTableSkeleton,
} = useGatewayRequestLogsResource();

const viewDetails = (entry: GatewayLogEntry) => {
  activeEntry.value = entry;
  isDetailsOpen.value = true;
};
const goToWAFTrace = (traceId?: string) => {
  if (!traceId) return;
  router.push(`/traces/${encodeURIComponent(traceId)}`);
};
const getEntrySelectionKey = (entry: GatewayLogEntry, index: number) =>
  buildGatewayLogSelectionKey(entry, index, currentCursor.value);
const getEntryIpSnapshot = (entry: GatewayLogEntry) =>
  getSnapshot(getEntryClientIp(entry));
const getEntryIpLocation = (entry: GatewayLogEntry) =>
  getEntryIpSnapshot(entry)?.location || entry.ipLocation || "";
const getEntryIpLocationText = (entry: GatewayLogEntry) => {
  const snapshot = getEntryIpSnapshot(entry);
  const location = snapshot?.location || entry.ipLocation || "";
  if (location) return location;
  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return t("admin.hostActiveIps.resolving");
  }
  if (snapshot?.status === "failed") {
    return t("admin.hostActiveIps.unavailable");
  }
  return "";
};
const getConnectionSourceText = (entry: GatewayLogEntry) => {
  const clientIp = getEntryClientIp(entry);
  const remoteIp = entry.remote_ip || "";
  if (!remoteIp || remoteIp === clientIp) return "";
  return t("admin.gatewayRequestLogs.connectionSource", { ip: remoteIp });
};
const displayedEntries = computed(() =>
  entries.value.map((entry, index) => ({
    ...entry,
    client_ip: getEntryClientIp(entry),
    ipLocation: getEntryIpLocation(entry),
    actionIp: getEntryActionIp(entry),
    selectionKey: getEntrySelectionKey(entry, index),
  })),
);

const {
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
} = useGatewayLogIpSelection({
  displayedEntries,
  selectedLogEntryKeys,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const activeEntryWithIpLocation = computed(() =>
  activeEntry.value
    ? {
        ...activeEntry.value,
        client_ip: getEntryClientIp(activeEntry.value),
        ipLocation: getEntryIpLocation(activeEntry.value),
      }
    : null,
);
const detailItems = computed(() =>
  buildGatewayLogDetailItems(
    activeEntryWithIpLocation.value,
    t,
    String(locale.value),
  ),
);
const detailCopyText = computed(() =>
  buildGatewayLogDetailCopyText(detailItems.value),
);
</script>

<template>
  <div class="flex h-full flex-col gap-3">
    <Teleport defer to="#request-analysis-logs-actions">
      <GatewayRequestLogsActions
        :block-ips="blockIpsFromLogs"
        :delete-selected-date="deleteSelectedDate"
        :is-blocking="isBlockingIps"
        :is-deleting="isDeleting"
        :is-mutating="isMutatingBlacklistIps"
        :is-releasing="isReleasingIps"
        :loading="loading"
        :refresh="refreshAll"
        :release-ips="releaseIpsFromLogs"
        :selected-blocked-ips="selectedBlockedLogIps"
        :selected-date="selectedDate"
        :selected-unblocked-ips="selectedUnblockedLogIps"
      />
    </Teleport>

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <GatewayRequestLogsFilters
        v-model:search-query="searchQuery"
        :active-credential-label="activeCredentialLabel"
        :active-logged-in-label="activeLoggedInLabel"
        :active-status-label="activeStatusLabel"
        :active-waf-status-label="activeWAFStatusLabel"
        :available-dates="availableDates"
        :credential-options="credentialFilterOptions"
        :cursor-page-label="cursorPageLabel"
        :entries-count="entries.length"
        :handle-credential-change="handleCredentialChange"
        :handle-date-change="handleDateChange"
        :handle-logged-in-change="handleLoggedInChange"
        :handle-search="handleSearch"
        :handle-status-change="handleStatusChange"
        :handle-waf-status-change="handleWAFStatusChange"
        :logs-dir="logsDir"
        :selected-credential="selectedCredential"
        :selected-date="selectedDate"
        :selected-logged-in="selectedLoggedIn"
        :selected-status="selectedStatus"
        :selected-waf-status="selectedWAFStatus"
      />

      <GatewayRequestLogsTable
        v-model:is-all-displayed-rows-selected="isAllDisplayedRowsSelected"
        :block-ips-from-logs="blockIpsFromLogs"
        :entries="displayedEntries"
        :entries-count="entries.length"
        :get-connection-source-text="getConnectionSourceText"
        :get-entry-ip-location-text="getEntryIpLocationText"
        :go-to-waf-trace="goToWAFTrace"
        :has-selectable-displayed-rows="hasSelectableDisplayedRows"
        :is-general-blacklisted="isGeneralBlacklisted"
        :is-mutating-blacklist-ips="isMutatingBlacklistIps"
        :loading="loading"
        :release-ips-from-logs="releaseIpsFromLogs"
        :selected-log-entry-keys="selectedLogEntryKeys"
        :show-table-skeleton="showTableSkeleton"
        :toggle-log-entry-selection="toggleLogEntrySelection"
        :view-details="viewDetails"
      />

      <GatewayRequestLogsPagination
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
      :title="t('admin.gatewayRequestLogs.detailTitle')"
      :description="t('admin.gatewayRequestLogs.detailDescription')"
      max-width-class="sm:max-w-[640px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEntry" class="space-y-4">
        <TraceIdLink
          :trace-id="activeEntry.trace_id || activeEntry.waf_trace_id"
        />
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>
