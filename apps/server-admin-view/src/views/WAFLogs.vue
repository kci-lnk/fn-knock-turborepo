<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Ban,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  Settings,
  ShieldAlert,
  Trash2,
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
import type { WAFEvent } from "../types";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { normalizeIpKey } from "../composables/useIpLocationBatch";
import { useWafLogIpSelection } from "./waf-logs/useWafLogIpSelection";
import { useWafLogDisplay } from "./waf-logs/useWafLogDisplay";
import {
  getWafEventSourceIp,
  useWafLogsResource,
} from "./waf-logs/useWafLogsResource";
import WAFLogsTable from "./waf-logs/WAFLogsTable.vue";

const LIMIT_OPTIONS = ["20", "50", "100", "200"] as const;

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
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2">
          <h2 class="text-lg font-semibold tracking-tight">
            {{ t("admin.wafLogs.title") }}
          </h2>
          <span class="text-xs text-muted-foreground">{{ selectedDate }}</span>
        </div>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.wafLogs.description") }}
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          @click="refreshAll"
        />
        <ConfirmDangerPopover
          v-if="selectedUnblockedWafIps.length > 0"
          :title="
            t('admin.wafLogs.blacklistSelectedTitle', {
              count: selectedUnblockedWafIps.length,
            })
          "
          :description="t('admin.wafLogs.blacklistDescription')"
          :loading="isBlockingIps"
          :disabled="
            selectedUnblockedWafIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => blockIpsFromWafLogs(selectedUnblockedWafIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
              :disabled="
                selectedUnblockedWafIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Ban class="mr-2 h-4 w-4" />
              {{
                t("admin.wafLogs.blacklistSelected", {
                  count: selectedUnblockedWafIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          v-if="selectedBlockedWafIps.length > 0"
          :title="
            t('admin.wafLogs.unblacklistSelectedTitle', {
              count: selectedBlockedWafIps.length,
            })
          "
          :description="t('admin.wafLogs.unblacklistDescription')"
          :loading="isReleasingIps"
          :disabled="
            selectedBlockedWafIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => releaseIpsFromWafLogs(selectedBlockedWafIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="text-foreground"
              :disabled="
                selectedBlockedWafIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Unlock class="mr-2 h-4 w-4" />
              {{
                t("admin.wafLogs.unblacklistSelected", {
                  count: selectedBlockedWafIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          :title="t('admin.wafLogs.deleteDateTitle', { date: selectedDate })"
          :description="t('admin.wafLogs.deleteDateDescription')"
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
              {{ t("admin.wafLogs.deleteDateAction") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <Alert
      v-if="!isWAFEnabled"
      class="flex items-center gap-3 rounded-lg border-dashed bg-muted/20 px-4 py-3 text-foreground shadow-none"
    >
      <ShieldAlert class="h-4 w-4 shrink-0 text-muted-foreground" />
      <div
        class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-sm text-muted-foreground">
          {{ t("admin.wafLogs.disabledNotice") }}
        </p>
        <Button variant="ghost" class="shrink-0" @click="goToSettings">
          <Settings class="mr-2 h-4 w-4" />
          {{ t("admin.wafLogs.goSettings") }}
        </Button>
      </div>
    </Alert>

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <div class="border-b px-4 py-3">
        <div class="flex flex-col gap-2 md:flex-row md:items-center">
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.wafLogs.searchPlaceholder')"
            class="w-full md:w-[320px] md:max-w-[320px]"
            @search="handleSearch"
          />

          <div class="flex flex-wrap items-center gap-2">
            <Select
              :model-value="selectedDate"
              @update:model-value="handleDateChange"
            >
              <div class="w-[148px]">
                <SelectTrigger :aria-label="t('admin.wafLogs.datePlaceholder')">
                  <SelectValue
                    :placeholder="t('admin.wafLogs.datePlaceholder')"
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
          </div>
        </div>

        <div
          class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
        >
          <span>
            {{ cursorPageLabel }} ·
            {{ t("admin.wafLogs.rowsCount", { count: entries.length }) }}
          </span>
          <span v-if="traceFilter.trim()" class="font-mono">{{
            t("admin.wafLogs.traceFilter", { trace: traceFilter.trim() })
          }}</span>
          <span v-if="searchQuery.trim()">{{
            t("admin.wafLogs.keywordFilter", {
              keyword: searchQuery.trim(),
            })
          }}</span>
        </div>
      </div>

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

      <FloatingActionDock
        :active="shouldFloatPagination"
        :keep-visible="loading && shouldFloatPagination"
        :keep-visible-release-delay="600"
        align="center"
        variant="surface"
        :visible-threshold="0.4"
        :aria-label="t('admin.wafLogs.title')"
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
                    ? t("admin.wafLogs.canLoadOlder")
                    : t("admin.wafLogs.lastPage")
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
                  {{ t("admin.wafLogs.firstPage") }}
                </Button>
                <Button
                  variant="outline"
                  class="h-8 px-3"
                  :disabled="loading || !canLoadNewer"
                  @click="handleLoadNewer"
                >
                  <ChevronLeft class="mr-1.5 h-4 w-4" />
                  {{ t("admin.wafLogs.previousPage") }}
                </Button>
                <Button
                  class="h-8 px-3"
                  :disabled="loading || !canLoadOlder"
                  @click="handleLoadOlder"
                >
                  {{ t("admin.wafLogs.nextPage") }}
                  <ChevronRight class="ml-1.5 h-4 w-4" />
                </Button>

                <div
                  class="ml-1 flex items-center gap-2 text-xs text-muted-foreground"
                >
                  <span>{{ t("admin.wafLogs.pageSize") }}</span>
                  <Select
                    :model-value="limit"
                    @update:model-value="handleLimitChange"
                  >
                    <div class="w-[96px]">
                      <SelectTrigger :aria-label="t('admin.wafLogs.pageSize')">
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
                          t("admin.wafLogs.pageSizeOption", {
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
                  {{ t("admin.wafLogs.firstPage") }}
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
                  {{ t("admin.wafLogs.previousPage") }}
                </span>
              </Button>
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button is-primary"
                :disabled="loading || !canLoadOlder"
                @click="handleLoadOlder"
              >
                <span>{{ t("admin.wafLogs.nextPage") }}</span>
                <ChevronRight class="h-4 w-4" />
              </Button>

              <Select
                :model-value="limit"
                @update:model-value="handleLimitChange"
              >
                <SelectTrigger
                  :aria-label="t('admin.wafLogs.pageSize')"
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
                    {{ t("admin.wafLogs.pageSizeOption", { count: option }) }}
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
