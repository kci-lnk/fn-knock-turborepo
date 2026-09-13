<script setup lang="ts">
import { toRef } from "vue";
import { useI18n } from "vue-i18n";
import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { useDashboardOnlineIps } from "./useDashboardOnlineIps";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { t, locale } = useI18n();
const {
  snapshot,
  loading,
  error,
  page,
  order,
  displayItems,
  ipCount,
  refresh,
  showPagination,
  hasPreviousPage,
  hasNextPage,
} = useDashboardOnlineIps(toRef(props, "open"));
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      :input-fullscreen="false"
      class="flex max-h-[90dvh] w-[calc(100%-1.5rem)] flex-col gap-0 overflow-hidden p-0 text-left sm:max-h-[85dvh] sm:max-w-[680px]"
    >
      <DialogHeader
        class="shrink-0 px-4 pb-2 pt-3 pr-11 text-left sm:px-5 sm:pr-12"
      >
        <div class="flex items-center justify-between gap-2">
          <DialogTitle class="text-base">{{
            t("admin.dashboard.onlineIps.title")
          }}</DialogTitle>
          <Button
            variant="ghost"
            size="icon"
            class="size-10 shrink-0"
            :disabled="loading"
            :aria-label="t('admin.dashboard.onlineIps.refresh')"
            :title="t('admin.dashboard.onlineIps.refresh')"
            @click="refresh"
          >
            <RefreshCw class="size-4" :class="{ 'animate-spin': loading }" />
          </Button>
        </div>
        <DialogDescription class="text-left text-xs">
          {{
            t("admin.dashboard.onlineIps.description", {
              seconds: snapshot?.window_seconds ?? 120,
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div
        v-if="snapshot?.items.length"
        class="flex shrink-0 items-center justify-between gap-2 border-b px-4 pb-2 sm:px-5"
      >
        <span class="min-w-0 text-xs text-muted-foreground" aria-live="polite">
          {{
            t("admin.dashboard.onlineIps.summary", {
              users: snapshot.online_count,
              ips: ipCount,
            })
          }}
        </span>
        <Button
          variant="ghost"
          size="sm"
          class="h-10 shrink-0 px-2 text-xs sm:h-8"
          :aria-pressed="order === 'asc'"
          @click="order = order === 'desc' ? 'asc' : 'desc'"
        >
          {{ t("admin.dashboard.onlineIps.lastActive") }}
          <component
            :is="order === 'desc' ? ArrowDown : ArrowUp"
            class="size-3.5"
          />
        </Button>
      </div>

      <div
        class="min-h-0 flex-1 overflow-y-auto overscroll-contain px-4 sm:px-5"
        :aria-busy="loading"
      >
        <div
          v-if="error"
          role="alert"
          class="my-3 rounded-md bg-destructive/5 p-3 text-sm text-destructive"
        >
          <p>{{ error }}</p>
          <p v-if="snapshot" class="mt-1 text-xs">
            {{ t("admin.dashboard.onlineIps.stale") }}
          </p>
          <Button
            class="mt-2"
            variant="outline"
            size="sm"
            :disabled="loading"
            @click="refresh"
          >
            {{ t("admin.dashboard.onlineIps.retry") }}
          </Button>
        </div>
        <div v-if="loading && !snapshot" class="space-y-3 py-4">
          <Skeleton v-for="row in 4" :key="row" class="h-14 w-full sm:h-8" />
        </div>
        <p
          v-else-if="snapshot && !snapshot.items.length"
          class="py-10 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.dashboard.onlineIps.empty") }}
        </p>
        <template v-else-if="snapshot">
          <div
            aria-hidden="true"
            class="online-ip-columns hidden gap-3 border-b py-2 text-xs text-muted-foreground sm:grid"
          >
            <span>IP</span
            ><span>{{ t("admin.dashboard.onlineIps.location") }}</span>
            <span>{{ t("admin.dashboard.onlineIps.lastActive") }}</span>
            <span class="text-right">{{
              t("admin.dashboard.onlineIps.identities")
            }}</span>
          </div>
          <ul
            class="divide-y"
            :aria-label="t('admin.dashboard.onlineIps.title')"
          >
            <li
              v-for="item in displayItems"
              :key="item.ip"
              class="online-ip-columns grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-1 py-3 text-xs sm:items-center"
            >
              <span
                class="col-span-2 min-w-0 break-all font-mono text-sm sm:col-span-1 sm:text-xs"
                >{{ item.ip || t("admin.dashboard.onlineIps.unknownIp") }}</span
              >
              <span
                class="col-span-2 min-w-0 break-words text-muted-foreground sm:col-span-1"
                >{{ item.locationText }}</span
              >
              <span class="min-w-0 text-muted-foreground">
                <span class="mr-1 sm:sr-only">{{
                  t("admin.dashboard.onlineIps.lastActive")
                }}</span>
                <HumanFriendlyTime
                  :value="item.last_seen_at"
                  :locale="locale"
                />
              </span>
              <span class="text-right tabular-nums text-muted-foreground">
                <span class="mr-1 sm:sr-only">{{
                  t("admin.dashboard.onlineIps.identities")
                }}</span
                >{{ item.identity_count }}
              </span>
            </li>
          </ul>
        </template>
      </div>

      <div
        v-if="showPagination"
        class="flex shrink-0 gap-3 border-t p-3 sm:justify-end sm:px-5"
        data-testid="online-ip-pagination"
      >
        <Button
          variant="outline"
          class="h-11 min-w-0 flex-1 sm:h-9 sm:flex-none"
          :disabled="!hasPreviousPage"
          @click="page--"
        >
          <ChevronLeft class="size-4" />{{ t("common.previousPage") }}
        </Button>
        <Button
          variant="outline"
          class="h-11 min-w-0 flex-1 sm:h-9 sm:flex-none"
          :disabled="!hasNextPage"
          @click="page++"
        >
          {{ t("common.nextPage") }}<ChevronRight class="size-4" />
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
@media (min-width: 640px) {
  .online-ip-columns {
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1.15fr) minmax(
        0,
        1fr
      ) 5rem;
  }
}
</style>
