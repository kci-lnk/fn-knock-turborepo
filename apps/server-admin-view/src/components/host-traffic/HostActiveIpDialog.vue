<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[85vh] flex-col overflow-hidden p-0 text-left sm:max-w-[680px]"
    >
      <DialogHeader class="shrink-0 border-b px-4 py-3 pr-10 text-left">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <DialogTitle class="truncate text-base" :title="displayTitle">
              {{ displayTitle }}
            </DialogTitle>
            <DialogDescription class="space-y-1 text-left">
              <span class="block break-all font-medium">{{ host }}</span>
              <span class="block text-xs">
                {{
                  t("admin.hostTraffic.activeIpDialog.description", {
                    range: activeWindowText,
                  })
                }}
              </span>
            </DialogDescription>
          </div>
          <Button
            variant="outline"
            size="sm"
            class="h-8 shrink-0 px-2.5 text-xs"
            :disabled="loading"
            @click="emit('refresh')"
          >
            <RefreshCw
              class="h-3.5 w-3.5"
              :class="{ 'animate-spin': loading }"
            />
            {{ t("admin.hostTraffic.activeIpDialog.refresh") }}
          </Button>
        </div>
      </DialogHeader>

      <div class="min-h-0 flex-1 overflow-y-auto p-4">
        <div
          class="mb-3 flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground"
        >
          <span>{{
            t("admin.hostTraffic.activeIpDialog.total", {
              count: items.length,
            })
          }}</span>
          <span v-if="updatedAt" class="inline-flex items-center gap-1">
            {{ t("admin.hostTraffic.activeIpDialog.updatedAt") }}
            <HumanFriendlyTime :value="updatedAt" :locale="locale" />
          </span>
        </div>

        <div
          v-if="loading && items.length === 0"
          class="space-y-2 rounded-md border p-3"
        >
          <Skeleton class="h-8 w-full rounded-md" />
          <Skeleton class="h-8 w-full rounded-md" />
          <Skeleton class="h-8 w-2/3 rounded-md" />
        </div>

        <div
          v-else-if="error"
          class="rounded-md border border-destructive/20 bg-destructive/5 px-3 py-6 text-center text-sm text-destructive"
        >
          {{ error }}
        </div>

        <div
          v-else-if="items.length === 0"
          class="rounded-md border px-3 py-8 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.hostTraffic.activeIpDialog.empty") }}
        </div>

        <div v-else class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow class="bg-muted/30">
                <TableHead class="w-[190px] text-xs">IP</TableHead>
                <TableHead class="text-xs">{{
                  t("admin.hostTraffic.activeIpDialog.location")
                }}</TableHead>
                <TableHead class="w-[120px] text-xs">{{
                  t("admin.hostTraffic.activeIpDialog.lastActive")
                }}</TableHead>
                <TableHead class="w-[88px] text-right text-xs">
                  {{ t("admin.hostTraffic.activeIpDialog.connections") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in items" :key="item.ip" class="align-top">
                <TableCell class="py-2.5">
                  <div class="font-mono text-xs leading-5">
                    {{ item.ip }}
                  </div>
                </TableCell>
                <TableCell class="py-2.5">
                  <div class="text-xs leading-5 text-muted-foreground">
                    {{ item.locationText }}
                  </div>
                </TableCell>
                <TableCell class="whitespace-nowrap py-2.5 text-xs">
                  <HumanFriendlyTime
                    :value="item.last_seen_at"
                    :locale="locale"
                  />
                </TableCell>
                <TableCell class="py-2.5 text-right">
                  <span
                    class="inline-flex min-w-8 justify-center rounded-full border px-2 py-0.5 text-xs text-muted-foreground"
                  >
                    {{ item.active_conns }}
                  </span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import type { HostActiveIpDisplayItem } from "../../composables/useHostActiveIps";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string | null;
    host: string;
    items: HostActiveIpDisplayItem[];
    loading?: boolean;
    error?: string;
    updatedAt?: number | null;
    windowSeconds?: number;
  }>(),
  {
    title: "",
    loading: false,
    error: "",
    updatedAt: null,
    windowSeconds: 120,
  },
);

const emit = defineEmits<{
  "update:open": [value: boolean];
  refresh: [];
}>();

const { t, locale } = useI18n();
const displayTitle = computed(
  () => props.title?.trim() || t("admin.hostTraffic.activeIpDialog.title"),
);

const activeWindowText = computed(() => {
  const seconds = Math.max(1, Number(props.windowSeconds || 120));
  if (seconds < 60) {
    return t("admin.hostTraffic.rangeSeconds", { count: seconds });
  }
  if (seconds < 3600) {
    return t("admin.hostTraffic.rangeMinutes", {
      count: Math.round(seconds / 60),
    });
  }
  if (seconds < 86400) {
    return t("admin.hostTraffic.rangeHours", {
      count: Math.round(seconds / 3600),
    });
  }
  return t("admin.hostTraffic.rangeDays", {
    count: Math.round(seconds / 86400),
  });
});
</script>
