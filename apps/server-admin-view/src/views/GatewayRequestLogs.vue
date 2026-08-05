<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Trash2,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  Ban,
  Unlock,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { GatewayLogEntry } from "../types";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import {
  LIMIT_OPTIONS,
  LOGIN_FILTER_OPTIONS,
  STATUS_FILTER_OPTIONS,
  WAF_FILTER_OPTIONS,
  buildGatewayLogDetailCopyText,
  buildGatewayLogDetailItems,
  buildGatewayLogSelectionKey,
  getEntryActionIp,
  getEntryClientIp,
} from "./gateway-request-logs/model";
import { useGatewayLogIpSelection } from "./gateway-request-logs/useGatewayLogIpSelection";
import { useGatewayRequestLogsResource } from "./gateway-request-logs/useGatewayRequestLogsResource";
import GatewayRequestLogsTable from "./gateway-request-logs/GatewayRequestLogsTable.vue";

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
  router.push({ path: "/waf-logs", query: { trace_id: traceId } });
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
      <div class="flex w-full flex-wrap items-center justify-end gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          class="px-2.5 [&_span]:hidden [&_svg]:mr-0 sm:px-3 sm:[&_span]:inline sm:[&_svg]:mr-1.5"
          @click="refreshAll"
        />
        <ConfirmDangerPopover
          v-if="selectedUnblockedLogIps.length > 0"
          :title="
            t('admin.gatewayRequestLogs.blacklistSelectedTitle', {
              count: selectedUnblockedLogIps.length,
            })
          "
          :description="t('admin.gatewayRequestLogs.blacklistDescription')"
          :loading="isBlockingIps"
          :disabled="
            selectedUnblockedLogIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => blockIpsFromLogs(selectedUnblockedLogIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/30 px-2.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive sm:px-4 sm:text-sm"
              :disabled="
                selectedUnblockedLogIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Ban class="mr-2 h-4 w-4" />
              {{
                t("admin.gatewayRequestLogs.blacklistSelected", {
                  count: selectedUnblockedLogIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          v-if="selectedBlockedLogIps.length > 0"
          :title="
            t('admin.gatewayRequestLogs.unblacklistSelectedTitle', {
              count: selectedBlockedLogIps.length,
            })
          "
          :description="t('admin.gatewayRequestLogs.unblacklistDescription')"
          :loading="isReleasingIps"
          :disabled="
            selectedBlockedLogIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => releaseIpsFromLogs(selectedBlockedLogIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="px-2.5 text-xs text-foreground sm:px-4 sm:text-sm"
              :disabled="
                selectedBlockedLogIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Unlock class="mr-2 h-4 w-4" />
              {{
                t("admin.gatewayRequestLogs.unblacklistSelected", {
                  count: selectedBlockedLogIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          :title="
            t('admin.gatewayRequestLogs.deleteDateTitle', {
              date: selectedDate,
            })
          "
          :description="t('admin.gatewayRequestLogs.deleteDateDescription')"
          :loading="isDeleting"
          :disabled="isDeleting"
          :on-confirm="deleteSelectedDate"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/30 px-2.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive sm:px-4 sm:text-sm"
              :disabled="isDeleting"
            >
              <Trash2 class="mr-2 h-4 w-4" />
              {{ t("admin.gatewayRequestLogs.deleteDateAction") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </Teleport>

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <div class="border-b px-3 py-3 sm:px-4">
        <div class="flex flex-col gap-2 lg:flex-row lg:items-start">
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.gatewayRequestLogs.searchPlaceholder')"
            class="w-full min-w-0 sm:w-[320px] lg:shrink-0"
            @search="handleSearch"
          />

          <div
            class="grid min-w-0 flex-1 grid-cols-2 items-center gap-2 sm:flex sm:flex-wrap sm:justify-end"
          >
            <Select
              :model-value="selectedDate"
              @update:model-value="handleDateChange"
            >
              <div
                class="order-1 w-full min-w-0 sm:order-none sm:w-[148px]"
              >
                <!-- prettier-ignore -->
                <SelectTrigger :aria-label="t('admin.gatewayRequestLogs.datePlaceholder')" class="w-full min-w-0">
                  <SelectValue
                    :placeholder="t('admin.gatewayRequestLogs.datePlaceholder')"
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="date in availableDates"
                  :key="date"
                  :value="date"
                >
                  {{ date }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedStatus"
              @update:model-value="handleStatusChange"
            >
              <div
                class="order-2 w-full min-w-0 sm:order-none sm:w-[156px]"
              >
                <SelectTrigger
                  :aria-label="t('admin.gatewayRequestLogs.statusPlaceholder')"
                  class="w-full min-w-0"
                >
                  <SelectValue
                    :placeholder="
                      t('admin.gatewayRequestLogs.statusPlaceholder')
                    "
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="option in STATUS_FILTER_OPTIONS"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ t(option.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedLoggedIn"
              @update:model-value="handleLoggedInChange"
            >
              <div
                class="order-3 w-full min-w-0 sm:order-none sm:w-[168px]"
              >
                <SelectTrigger
                  :aria-label="t('admin.gatewayRequestLogs.loginPlaceholder')"
                  class="w-full min-w-0"
                >
                  <SelectValue
                    :placeholder="
                      t('admin.gatewayRequestLogs.loginPlaceholder')
                    "
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="option in LOGIN_FILTER_OPTIONS"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ t(option.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedCredential"
              @update:model-value="handleCredentialChange"
            >
              <div
                class="order-5 col-span-2 w-full min-w-0 sm:order-none sm:col-span-1 sm:w-[220px]"
              >
                <SelectTrigger
                  :aria-label="
                    t('admin.gatewayRequestLogs.credentialPlaceholder')
                  "
                  class="w-full min-w-0"
                >
                  <SelectValue
                    :placeholder="
                      t('admin.gatewayRequestLogs.credentialPlaceholder')
                    "
                  />
                </SelectTrigger>
              </div>
              <SelectContent class="max-w-[min(28rem,calc(100vw-2rem))]">
                <SelectItem
                  v-for="option in credentialFilterOptions"
                  :key="option.value"
                  :value="option.value"
                  class="min-w-0"
                >
                  <span
                    class="block max-w-[22rem] truncate"
                    :title="option.label"
                  >
                    {{ option.label }}
                  </span>
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedWAFStatus"
              @update:model-value="handleWAFStatusChange"
            >
              <div
                class="order-4 w-full min-w-0 sm:order-none sm:w-[144px]"
              >
                <SelectTrigger
                  :aria-label="t('admin.gatewayRequestLogs.wafPlaceholder')"
                  class="w-full min-w-0"
                >
                  <SelectValue
                    :placeholder="t('admin.gatewayRequestLogs.wafPlaceholder')"
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="option in WAF_FILTER_OPTIONS"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ t(option.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div
          class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
        >
          <span>
            {{ cursorPageLabel }} ·
            {{
              t("admin.gatewayRequestLogs.rowsCount", {
                count: entries.length,
              })
            }}
          </span>
          <span>{{ activeStatusLabel }}</span>
          <span>{{ activeLoggedInLabel }}</span>
          <span class="max-w-[220px] truncate" :title="activeCredentialLabel">
            {{ activeCredentialLabel }}
          </span>
          <span>{{ activeWAFStatusLabel }}</span>
          <span v-if="searchQuery.trim()">{{
            t("admin.gatewayRequestLogs.keywordFilter", {
              keyword: searchQuery.trim(),
            })
          }}</span>
          <span class="hidden break-all sm:inline">{{
            t("admin.gatewayRequestLogs.directoryLabel", {
              directory: logsDir || "-",
            })
          }}</span>
        </div>
      </div>

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

      <FloatingActionDock
        :active="shouldFloatPagination"
        :keep-visible="loading && shouldFloatPagination"
        :keep-visible-release-delay="600"
        align="center"
        variant="surface"
        :visible-threshold="0.4"
        :aria-label="t('admin.gatewayRequestLogs.title')"
        floating-class="min-w-0 max-w-[calc(100vw-2rem)] rounded-[1.25rem] p-2"
      >
        <template #inline>
          <div class="border-t px-3 py-3 sm:px-4">
            <div
              class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
            >
              <div
                class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground"
              >
                <span>{{ cursorPageLabel }}</span>
                <span>{{
                  canLoadOlder
                    ? t("admin.gatewayRequestLogs.canLoadOlder")
                    : t("admin.gatewayRequestLogs.lastPage")
                }}</span>
              </div>

              <div class="flex flex-wrap items-center justify-end gap-2">
                <Button
                  variant="outline"
                  class="h-8 px-2.5 sm:px-3"
                  :aria-label="t('admin.gatewayRequestLogs.firstPage')"
                  :disabled="loading || !canLoadNewer"
                  @click="handleLoadFirst"
                >
                  <ChevronsLeft class="h-4 w-4 sm:mr-1.5" />
                  <span class="hidden sm:inline">{{
                    t("admin.gatewayRequestLogs.firstPage")
                  }}</span>
                </Button>
                <Button
                  variant="outline"
                  class="h-8 px-2.5 sm:px-3"
                  :aria-label="t('admin.gatewayRequestLogs.previousPage')"
                  :disabled="loading || !canLoadNewer"
                  @click="handleLoadNewer"
                >
                  <ChevronLeft class="h-4 w-4 sm:mr-1.5" />
                  <span class="hidden sm:inline">{{
                    t("admin.gatewayRequestLogs.previousPage")
                  }}</span>
                </Button>
                <Button
                  class="h-8 px-3"
                  :disabled="loading || !canLoadOlder"
                  @click="handleLoadOlder"
                >
                  {{ t("admin.gatewayRequestLogs.nextPage") }}
                  <ChevronRight class="ml-1.5 h-4 w-4" />
                </Button>

                <div
                  class="flex items-center gap-2 text-xs text-muted-foreground sm:ml-1"
                >
                  <span>{{ t("admin.gatewayRequestLogs.pageSize") }}</span>
                  <Select
                    :model-value="limit"
                    @update:model-value="handleLimitChange"
                  >
                    <div class="w-[96px]">
                      <SelectTrigger
                        :aria-label="t('admin.gatewayRequestLogs.pageSize')"
                      >
                        <SelectValue />
                      </SelectTrigger>
                    </div>
                    <SelectContent>
                      <SelectItem
                        v-for="option in LIMIT_OPTIONS"
                        :key="option"
                        :value="option"
                      >
                        {{
                          t("admin.gatewayRequestLogs.pageSizeOption", {
                            count: option,
                          })
                        }}
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </div>
          </div>
        </template>

        <template #floating>
          <div class="floating-cursor-pagination">
            <div class="floating-cursor-pagination__controls">
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button"
                :disabled="loading || !canLoadNewer"
                @click="handleLoadFirst"
              >
                <ChevronsLeft class="h-4 w-4" />
                <span class="hidden sm:inline">
                  {{ t("admin.gatewayRequestLogs.firstPage") }}
                </span>
              </Button>
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button"
                :disabled="loading || !canLoadNewer"
                @click="handleLoadNewer"
              >
                <ChevronLeft class="h-4 w-4" />
                <span class="hidden sm:inline">
                  {{ t("admin.gatewayRequestLogs.previousPage") }}
                </span>
              </Button>
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button is-primary"
                :disabled="loading || !canLoadOlder"
                @click="handleLoadOlder"
              >
                <span>{{ t("admin.gatewayRequestLogs.nextPage") }}</span>
                <ChevronRight class="h-4 w-4" />
              </Button>

              <Select
                :model-value="limit"
                @update:model-value="handleLimitChange"
              >
                <SelectTrigger
                  :aria-label="t('admin.gatewayRequestLogs.pageSize')"
                  class="h-9 w-[84px] rounded-xl border-white/10 bg-white/10 text-white shadow-none hover:bg-white/15 focus-visible:border-white/30 focus-visible:ring-white/20 [&_svg]:text-white"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in LIMIT_OPTIONS"
                    :key="option"
                    :value="option"
                  >
                    {{
                      t("admin.gatewayRequestLogs.pageSizeOption", {
                        count: option,
                      })
                    }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </template>
      </FloatingActionDock>
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      :title="t('admin.gatewayRequestLogs.detailTitle')"
      :description="t('admin.gatewayRequestLogs.detailDescription')"
      max-width-class="sm:max-w-[640px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEntry">
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>

<style scoped>
.floating-cursor-pagination {
  display: flex;
  max-width: calc(100vw - 3rem);
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.6rem 0.8rem;
}

.floating-cursor-pagination__controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
}

:deep(.floating-cursor-pagination__button) {
  height: 2.25rem;
  min-width: 2.25rem;
  border-color: transparent;
  border-radius: 0.8rem;
  background: transparent;
  padding-inline: 0.7rem;
  color: rgb(255 255 255 / 82%);
  box-shadow: none;
}

:deep(.floating-cursor-pagination__button:hover) {
  background: rgb(255 255 255 / 12%);
  color: #fff;
}

:deep(.floating-cursor-pagination__button.is-primary) {
  background: #fff;
  color: #09090b;
}

:deep(.floating-cursor-pagination__button.is-primary:hover) {
  background: rgb(255 255 255 / 92%);
  color: #09090b;
}

:deep(.floating-cursor-pagination__button:disabled) {
  opacity: 0.46;
}
</style>
