<script setup lang="ts">
import { toRef } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowDown, ArrowUp, RefreshCw, Users } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Skeleton } from "@/components/ui/skeleton";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import { useDashboardOnlineIps } from "./useDashboardOnlineIps";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { t, locale } = useI18n();
const {
  snapshot,
  loading,
  error,
  page,
  limit,
  order,
  pageSize,
  displayItems,
  ipCount,
  refresh,
} = useDashboardOnlineIps(toRef(props, "open"));
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      :input-fullscreen="false"
      class="flex max-h-[85dvh] w-[calc(100%-2rem)] flex-col gap-0 overflow-hidden p-0 text-left sm:max-w-[760px]"
    >
      <DialogHeader class="shrink-0 border-b px-5 py-4 pr-12 text-left">
        <DialogTitle class="flex items-center gap-2 text-base">
          <Users class="size-4 text-muted-foreground" />
          {{ t("admin.dashboard.onlineIps.title") }}
        </DialogTitle>
        <DialogDescription class="text-left text-xs leading-relaxed">
          {{
            t("admin.dashboard.onlineIps.description", {
              seconds: snapshot?.window_seconds ?? 120,
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div
        class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b bg-muted/20 px-5 py-3"
      >
        <div class="space-y-1" aria-live="polite">
          <p class="text-sm font-medium">
            {{
              t("admin.dashboard.onlineIps.summary", {
                users: snapshot?.online_count ?? "—",
                ips: snapshot ? ipCount : "—",
              })
            }}
          </p>
          <p
            v-if="snapshot"
            class="flex flex-wrap items-center gap-1 text-xs text-muted-foreground"
          >
            {{ t("admin.dashboard.onlineIps.snapshotAt") }}
            <HumanFriendlyTime :value="snapshot.timestamp" :locale="locale" />
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          :disabled="loading"
          @click="refresh"
        >
          <RefreshCw class="size-3.5" :class="{ 'animate-spin': loading }" />
          {{ t("admin.dashboard.onlineIps.refresh") }}
        </Button>
      </div>

      <div class="min-h-0 flex-1 overflow-auto p-4 sm:p-5" :aria-busy="loading">
        <div
          v-if="error"
          role="alert"
          class="mb-3 rounded-md border border-destructive/20 bg-destructive/5 p-3 text-sm text-destructive"
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
        <div
          v-if="loading && !snapshot"
          class="space-y-3 rounded-md border p-4"
        >
          <Skeleton v-for="row in 5" :key="row" class="h-8 w-full" />
        </div>
        <div
          v-else-if="snapshot && !snapshot.items.length"
          class="rounded-md border px-4 py-12 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.dashboard.onlineIps.empty") }}
        </div>
        <div v-else-if="snapshot" class="overflow-x-auto rounded-md border">
          <Table class="min-w-[560px]">
            <TableHeader>
              <TableRow class="bg-muted/30">
                <TableHead class="w-[190px]">IP</TableHead>
                <TableHead>{{
                  t("admin.dashboard.onlineIps.location")
                }}</TableHead>
                <TableHead
                  :aria-sort="order === 'desc' ? 'descending' : 'ascending'"
                  class="w-[140px]"
                >
                  <button
                    type="button"
                    class="inline-flex items-center gap-1 rounded-sm py-3 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    @click="order = order === 'desc' ? 'asc' : 'desc'"
                  >
                    {{ t("admin.dashboard.onlineIps.lastActive") }}
                    <ArrowDown v-if="order === 'desc'" class="size-3.5" />
                    <ArrowUp v-else class="size-3.5" />
                  </button>
                </TableHead>
                <TableHead class="w-[100px] text-right">{{
                  t("admin.dashboard.onlineIps.identities")
                }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in displayItems" :key="item.ip">
                <TableCell class="break-all font-mono text-xs">{{
                  item.ip || t("admin.dashboard.onlineIps.unknownIp")
                }}</TableCell>
                <TableCell class="text-xs text-muted-foreground">{{
                  item.locationText
                }}</TableCell>
                <TableCell class="whitespace-nowrap text-xs"
                  ><HumanFriendlyTime
                    :value="item.last_seen_at"
                    :locale="locale"
                /></TableCell>
                <TableCell class="text-right text-xs tabular-nums">{{
                  item.identity_count
                }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </div>
      <div
        v-if="snapshot?.items.length"
        class="shrink-0 border-t [&>div]:flex-wrap [&>div]:gap-3 [&>div]:border-t-0 [&>div>div]:flex-wrap [&>div>div]:gap-2"
      >
        <PagedTableFooter
          v-model:page="page"
          v-model:limit="limit"
          :total="snapshot.items.length"
          :items-per-page="pageSize"
        />
      </div>
    </DialogContent>
  </Dialog>
</template>
