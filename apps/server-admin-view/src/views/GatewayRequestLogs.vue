<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Info,
  Settings,
  Trash2,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  Ban,
  Unlock,
} from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
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
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { docsUrls } from "../lib/docs";
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
  isLoggingEnabled,
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

const goToSettings = () => {
  router.push({ path: "/system", query: { tab: "gateway-logging" } });
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
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2">
          <h1 class="text-lg font-semibold tracking-tight">
            {{ t("admin.gatewayRequestLogs.title") }}
          </h1>
          <span class="text-xs text-muted-foreground">{{ selectedDate }}</span>
        </div>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.description") }}
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <DocsLinkButton :href="docsUrls.guides.requestLogs" />
        <RefreshButton
          :loading="loading"
          :disabled="loading"
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
              class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
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
              class="text-foreground"
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
              class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
              :disabled="isDeleting"
            >
              <Trash2 class="mr-2 h-4 w-4" />
              {{ t("admin.gatewayRequestLogs.deleteDateAction") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <Alert
      v-if="!isLoggingEnabled"
      class="flex items-center gap-3 rounded-lg border-dashed bg-muted/20 px-4 py-3 text-foreground shadow-none"
    >
      <Info class="h-4 w-4 shrink-0 text-muted-foreground" />
      <div
        class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.disabledNotice") }}
        </p>
        <Button variant="ghost" class="shrink-0" @click="goToSettings">
          <Settings class="mr-2 h-4 w-4" />
          {{ t("admin.gatewayRequestLogs.goSettings") }}
        </Button>
      </div>
    </Alert>

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <div class="border-b px-4 py-3">
        <div class="flex flex-col gap-2 lg:flex-row lg:items-start">
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.gatewayRequestLogs.searchPlaceholder')"
            class="w-full min-w-0 sm:w-[320px] lg:shrink-0"
            @search="handleSearch"
          />

          <div
            class="flex min-w-0 flex-1 flex-wrap items-center gap-2 sm:justify-end"
          >
            <Select
              :model-value="selectedDate"
              @update:model-value="handleDateChange"
            >
              <div class="w-full min-w-0 sm:w-[148px]">
                <SelectTrigger class="w-full min-w-0">
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
              <div class="w-full min-w-0 sm:w-[156px]">
                <SelectTrigger class="w-full min-w-0">
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
              <div class="w-full min-w-0 sm:w-[168px]">
                <SelectTrigger class="w-full min-w-0">
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
              <div class="w-full min-w-0 sm:w-[220px]">
                <SelectTrigger class="w-full min-w-0">
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
              <div class="w-full min-w-0 sm:w-[144px]">
                <SelectTrigger class="w-full min-w-0">
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
          <span class="break-all">{{
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
          <div class="border-t px-4 py-3">
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
                  class="h-8 px-3"
                  :disabled="loading || !canLoadNewer"
                  @click="handleLoadFirst"
                >
                  <ChevronsLeft class="mr-1.5 h-4 w-4" />
                  {{ t("admin.gatewayRequestLogs.firstPage") }}
                </Button>
                <Button
                  variant="outline"
                  class="h-8 px-3"
                  :disabled="loading || !canLoadNewer"
                  @click="handleLoadNewer"
                >
                  <ChevronLeft class="mr-1.5 h-4 w-4" />
                  {{ t("admin.gatewayRequestLogs.previousPage") }}
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
                  class="ml-1 flex items-center gap-2 text-xs text-muted-foreground"
                >
                  <span>{{ t("admin.gatewayRequestLogs.pageSize") }}</span>
                  <Select
                    :model-value="limit"
                    @update:model-value="handleLimitChange"
                  >
                    <div class="w-[96px]">
                      <SelectTrigger>
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
