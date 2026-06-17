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
          <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
            <ConfirmDangerPopover
              v-if="selectedUnblockedIps.length > 0"
              :title="
                t('admin.hostTraffic.activeIpDialog.blacklistSelectedTitle', {
                  count: selectedUnblockedIps.length,
                })
              "
              :description="
                t('admin.hostTraffic.activeIpDialog.blacklistDescription')
              "
              :loading="isBlockingIps"
              :disabled="
                selectedUnblockedIps.length === 0 || isMutatingBlacklistIps
              "
              :on-confirm="() => blockIps(selectedUnblockedIps)"
            >
              <template #trigger>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-8 border-destructive/30 px-2.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
                  :disabled="
                    selectedUnblockedIps.length === 0 ||
                    isMutatingBlacklistIps
                  "
                >
                  <Ban class="h-3.5 w-3.5" />
                  {{
                    t("admin.hostTraffic.activeIpDialog.blacklistSelected", {
                      count: selectedUnblockedIps.length,
                    })
                  }}
                </Button>
              </template>
            </ConfirmDangerPopover>
            <ConfirmDangerPopover
              v-if="selectedBlockedIps.length > 0"
              :title="
                t('admin.hostTraffic.activeIpDialog.unblacklistSelectedTitle', {
                  count: selectedBlockedIps.length,
                })
              "
              :description="
                t('admin.hostTraffic.activeIpDialog.unblacklistDescription')
              "
              :loading="isReleasingIps"
              :disabled="
                selectedBlockedIps.length === 0 || isMutatingBlacklistIps
              "
              :on-confirm="() => releaseIps(selectedBlockedIps)"
            >
              <template #trigger>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-8 px-2.5 text-xs"
                  :disabled="
                    selectedBlockedIps.length === 0 || isMutatingBlacklistIps
                  "
                >
                  <Unlock class="h-3.5 w-3.5" />
                  {{
                    t("admin.hostTraffic.activeIpDialog.unblacklistSelected", {
                      count: selectedBlockedIps.length,
                    })
                  }}
                </Button>
              </template>
            </ConfirmDangerPopover>
            <Button
              variant="outline"
              size="sm"
              class="h-8 px-2.5 text-xs"
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

        <div v-else class="overflow-x-auto rounded-md border">
          <Table class="min-w-[640px]">
            <TableHeader>
              <TableRow class="bg-muted/30">
                <TableHead class="w-[44px] text-xs">
                  <Checkbox
                    v-model="isAllSelected"
                    :disabled="visibleIps.length === 0"
                  />
                </TableHead>
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
                <TableHead class="w-[72px] text-right text-xs">
                  {{ t("admin.sessions.table.actions") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in items" :key="item.ip" class="align-top">
                <TableCell class="py-2.5">
                  <Checkbox
                    :model-value="selectedIps.has(item.ip)"
                    @update:model-value="toggleSelect(item.ip)"
                  />
                </TableCell>
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
                <TableCell class="py-2.5 text-right">
                  <ConfirmDangerPopover
                    :title="
                      isGeneralBlacklisted(item.ip)
                        ? t('admin.hostTraffic.activeIpDialog.unblacklistOneTitle')
                        : t('admin.hostTraffic.activeIpDialog.blacklistOneTitle')
                    "
                    :description="
                      isGeneralBlacklisted(item.ip)
                        ? t(
                            'admin.hostTraffic.activeIpDialog.unblacklistOneDescription',
                            {
                              ip: item.ip,
                            },
                          )
                        : t(
                            'admin.hostTraffic.activeIpDialog.blacklistOneDescription',
                            {
                              ip: item.ip,
                            },
                          )
                    "
                    :loading="isMutatingBlacklistIps"
                    :disabled="isMutatingBlacklistIps"
                    :on-confirm="
                      () =>
                        isGeneralBlacklisted(item.ip)
                          ? releaseIps([item.ip])
                          : blockIps([item.ip])
                    "
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8"
                        :class="
                          isGeneralBlacklisted(item.ip)
                            ? 'text-foreground hover:text-foreground'
                            : 'text-destructive hover:text-destructive'
                        "
                        :disabled="isMutatingBlacklistIps"
                        :aria-label="
                          isGeneralBlacklisted(item.ip)
                            ? t('admin.hostTraffic.activeIpDialog.unblacklistOne')
                            : t('admin.hostTraffic.activeIpDialog.blacklistOne')
                        "
                      >
                        <Unlock
                          v-if="isGeneralBlacklisted(item.ip)"
                          class="h-4 w-4"
                        />
                        <Ban v-else class="h-4 w-4" />
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
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
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Ban, RefreshCw, Unlock } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import type { HostActiveIpDisplayItem } from "../../composables/useHostActiveIps";
import { useGeneralBlacklistStatus } from "../../composables/useGeneralBlacklistStatus";
import { GeneralBlacklistAPI } from "../../lib/api";

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
const selectedIps = ref<Set<string>>(new Set());
const displayTitle = computed(
  () => props.title?.trim() || t("admin.hostTraffic.activeIpDialog.title"),
);

const { isPending: isBlockingIps, run: runBlockIps } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.hostTraffic.activeIpDialog.blacklistFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.hostTraffic.activeIpDialog.blacklistFailed"),
      ),
    });
  },
});
const { isPending: isReleasingIps, run: runReleaseIps } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.hostTraffic.activeIpDialog.unblacklistFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.hostTraffic.activeIpDialog.unblacklistFailed"),
      ),
    });
  },
});
const isMutatingBlacklistIps = computed(
  () => isBlockingIps.value || isReleasingIps.value,
);

const visibleIps = computed(() =>
  Array.from(new Set(props.items.map((item) => item.ip).filter(Boolean))),
);
const {
  refresh: refreshGeneralBlacklistStatus,
  isBlacklisted: isGeneralBlacklisted,
} = useGeneralBlacklistStatus(visibleIps);
const selectedIpList = computed(() => Array.from(selectedIps.value));
const selectedBlockedIps = computed(() =>
  selectedIpList.value.filter((ip) => isGeneralBlacklisted(ip)),
);
const selectedUnblockedIps = computed(() =>
  selectedIpList.value.filter((ip) => !isGeneralBlacklisted(ip)),
);

const isAllSelected = computed({
  get: () =>
    visibleIps.value.length > 0 &&
    visibleIps.value.every((ip) => selectedIps.value.has(ip)),
  set: (checked: boolean) => {
    const next = new Set(selectedIps.value);
    if (checked) {
      visibleIps.value.forEach((ip) => next.add(ip));
    } else {
      visibleIps.value.forEach((ip) => next.delete(ip));
    }
    selectedIps.value = next;
  },
});

const toggleSelect = (ip: string) => {
  const next = new Set(selectedIps.value);
  if (next.has(ip)) {
    next.delete(ip);
  } else {
    next.add(ip);
  }
  selectedIps.value = next;
};

const removeSelectedIps = (ips: string[]) => {
  const operatedIps = new Set(ips);
  selectedIps.value = new Set(
    Array.from(selectedIps.value).filter((ip) => !operatedIps.has(ip)),
  );
};

const blockIps = async (ips: string[]) => {
  const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter(
    (ip) => !isGeneralBlacklisted(ip),
  );
  if (uniqueIps.length === 0) return;

  await runBlockIps(() => GeneralBlacklistAPI.add(uniqueIps, "active_ip"), {
    onSuccess: async (result) => {
      toast.success(t("admin.hostTraffic.activeIpDialog.blacklistSuccess"), {
        description: t(
          "admin.hostTraffic.activeIpDialog.blacklistSuccessDetail",
          {
            added: result?.added ?? 0,
            updated: result?.updated ?? 0,
          },
        ),
      });
      removeSelectedIps(uniqueIps);
      await refreshGeneralBlacklistStatus();
      emit("refresh");
    },
  });
};

const releaseIps = async (ips: string[]) => {
  const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter((ip) =>
    isGeneralBlacklisted(ip),
  );
  if (uniqueIps.length === 0) return;

  await runReleaseIps(() => GeneralBlacklistAPI.delete(uniqueIps), {
    onSuccess: async (result) => {
      toast.success(t("admin.hostTraffic.activeIpDialog.unblacklistSuccess"), {
        description: t(
          "admin.hostTraffic.activeIpDialog.unblacklistSuccessDetail",
          {
            removed: result?.removed ?? 0,
          },
        ),
      });
      removeSelectedIps(uniqueIps);
      await refreshGeneralBlacklistStatus();
      emit("refresh");
    },
  });
};

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

watch(
  () => props.open,
  (open) => {
    if (!open) {
      selectedIps.value = new Set();
    }
  },
);

watch(visibleIps, (ips) => {
  const visible = new Set(ips);
  selectedIps.value = new Set(
    Array.from(selectedIps.value).filter((ip) => visible.has(ip)),
  );
});
</script>
