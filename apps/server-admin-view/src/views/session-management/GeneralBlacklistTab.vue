<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import { toast } from "@admin-shared/utils/toast";
import { Ban, Loader2, Plus, Trash2 } from "lucide-vue-next";
import { useIpLocationBatch } from "../../composables/useIpLocationBatch";
import {
  GeneralBlacklistAPI,
  type GeneralBlacklistRecord,
  type GeneralBlacklistSource,
} from "../../lib/api";

const { t, locale } = useI18n();
const addDialogOpen = ref(false);
const addIpsText = ref("");
const addComment = ref("");

const {
  items: records,
  total: totalRecords,
  loading,
  searchQuery,
  currentPage,
  limit,
  parsedLimit,
  selectedKeys: selectedIps,
  isAllSelected,
  fetchList: fetchBlacklist,
  handleSearch,
  handlePageChange,
  handleLimitChange,
  toggleSelect,
  clearSelection,
} = usePagedSelectionList<GeneralBlacklistRecord, string>({
  fetchPage: async ({ page, limit, query }) => {
    const data = await GeneralBlacklistAPI.getList(page, limit, query);
    return {
      items: data.items || [],
      total: data.total || 0,
    };
  },
  getKey: (record) => record.ip,
  onError: (error) => {
    toast.error(t("admin.sessions.generalBlacklist.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.generalBlacklist.loadFailed"),
      ),
    });
  },
});

const { trackIps, getSnapshot } = useIpLocationBatch();
const { isPending: isAdding, run: runAddAction } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.generalBlacklist.addFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.generalBlacklist.addFailed"),
      ),
    });
  },
});
const { isPending: isDeleting, run: runDeleteAction } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.generalBlacklist.deleteFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.generalBlacklist.deleteFailed"),
      ),
    });
  },
});

const showTableSkeleton = useDelayedLoading(
  () => loading.value && records.value.length === 0,
);

const parsedAddIps = computed(() =>
  Array.from(
    new Set(
      addIpsText.value
        .split(/[\s,;]+/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  ),
);

const getSourceLabel = (source?: string) => {
  switch (source) {
    case "request_log":
      return t("admin.sessions.generalBlacklist.sources.requestLog");
    case "active_ip":
      return t("admin.sessions.generalBlacklist.sources.activeIp");
    case "waf_log":
      return t("admin.sessions.generalBlacklist.sources.wafLog");
    default:
      return t("admin.sessions.generalBlacklist.sources.manual");
  }
};

const getSourceVariant = (source?: string) => {
  if (source === "request_log") return "secondary";
  if (source === "active_ip") return "outline";
  if (source === "waf_log") return "destructive";
  return "default";
};

const getLocationText = (ip: string) => {
  const snapshot = getSnapshot(ip);
  if (snapshot?.location) return snapshot.location;
  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return t("admin.hostActiveIps.resolving");
  }
  if (snapshot?.status === "skipped") {
    return t("admin.hostActiveIps.privateAddress");
  }
  return t("admin.hostActiveIps.unavailable");
};

const openAddDialog = () => {
  addDialogOpen.value = true;
};

const addBlacklist = async (
  ips: string[],
  source: GeneralBlacklistSource,
  comment?: string,
) => {
  if (ips.length === 0) return;

  await runAddAction(() => GeneralBlacklistAPI.add(ips, source, comment), {
    onSuccess: async (result) => {
      toast.success(t("admin.sessions.generalBlacklist.addSuccess"), {
        description: t("admin.sessions.generalBlacklist.addSuccessDetail", {
          added: result?.added ?? 0,
          updated: result?.updated ?? 0,
        }),
      });
      addDialogOpen.value = false;
      addIpsText.value = "";
      addComment.value = "";
      await fetchBlacklist();
    },
  });
};

const addManualBlacklist = async () => {
  await addBlacklist(parsedAddIps.value, "manual", addComment.value.trim());
};

const deleteBlacklist = async (ips: string[]) => {
  if (ips.length === 0) return;

  await runDeleteAction(() => GeneralBlacklistAPI.delete(ips), {
    onSuccess: async (result) => {
      toast.success(t("admin.sessions.generalBlacklist.deleteSuccess"), {
        description: t("admin.sessions.generalBlacklist.deleteSuccessDetail", {
          removed: result?.removed ?? 0,
        }),
      });
      clearSelection();
      await fetchBlacklist();
    },
  });
};

const deleteOne = async (ip: string) => {
  await runDeleteAction(() => GeneralBlacklistAPI.deleteByIp(ip), {
    onSuccess: async () => {
      toast.success(t("admin.sessions.generalBlacklist.deleteSuccess"));
      selectedIps.value.delete(ip);
      selectedIps.value = new Set(selectedIps.value);
      await fetchBlacklist();
    },
  });
};

watch(
  records,
  (items) => {
    trackIps(items.map((record) => record.ip));
  },
  { immediate: true },
);

onMounted(() => {
  fetchBlacklist();
});
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <div class="flex flex-col gap-3 lg:flex-row lg:items-center">
      <SearchInput
        v-model="searchQuery"
        :placeholder="t('admin.sessions.generalBlacklist.searchPlaceholder')"
        class="w-full lg:w-[280px]"
        @search="handleSearch"
      />
      <div class="flex-1"></div>
      <div class="flex flex-wrap items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          @click="fetchBlacklist"
        />
        <Button @click="openAddDialog">
          <Plus class="h-4" />
          {{ t("admin.sessions.generalBlacklist.addButton") }}
        </Button>
        <ConfirmDangerPopover
          :title="
            t('admin.sessions.generalBlacklist.deleteSelectedTitle', {
              count: selectedIps.size,
            })
          "
          :description="t('admin.sessions.generalBlacklist.deleteDescription')"
          :loading="isDeleting"
          :disabled="selectedIps.size === 0 || isDeleting"
          :on-confirm="() => deleteBlacklist(Array.from(selectedIps))"
        >
          <template #trigger>
            <Button
              variant="destructive"
              :disabled="selectedIps.size === 0 || isDeleting"
            >
              <Trash2 class="h-4" />
              {{
                t("admin.sessions.generalBlacklist.deleteSelected", {
                  count: selectedIps.size,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <div
      class="border rounded-md overflow-hidden bg-background flex-1 flex flex-col"
    >
      <div class="flex-1 w-full overflow-hidden">
        <div class="h-full overflow-auto">
          <Table v-if="!(loading && records.length === 0)">
            <TableHeader class="sticky top-0 bg-background z-10 shadow-sm">
              <TableRow>
                <TableHead class="w-[50px]">
                  <Checkbox v-model="isAllSelected" />
                </TableHead>
                <TableHead>{{
                  t("admin.sessions.generalBlacklist.ipLocationHeader")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.generalBlacklist.source")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.generalBlacklist.comment")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.generalBlacklist.createdAt")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.generalBlacklist.updatedAt")
                }}</TableHead>
                <TableHead class="text-right pr-6">{{
                  t("admin.sessions.table.actions")
                }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="7" class="text-center py-10">
                  <Loader2
                    class="h-6 w-6 animate-spin mx-auto text-muted-foreground"
                  />
                </TableCell>
              </TableRow>
              <TableRow v-else-if="records.length === 0">
                <TableCell
                  colspan="7"
                  class="text-center py-10 text-muted-foreground"
                >
                  {{ t("admin.sessions.generalBlacklist.empty") }}
                </TableCell>
              </TableRow>
              <TableRow v-else v-for="record in records" :key="record.ip">
                <TableCell>
                  <Checkbox
                    :model-value="selectedIps.has(record.ip)"
                    @update:model-value="toggleSelect(record.ip)"
                  />
                </TableCell>
                <TableCell class="font-medium">
                  <div class="font-mono text-sm break-all">{{ record.ip }}</div>
                  <div class="text-xs text-muted-foreground mt-0.5 break-all">
                    {{ record.ipLocation || getLocationText(record.ip) }}
                  </div>
                </TableCell>
                <TableCell>
                  <Badge :variant="getSourceVariant(record.source)">
                    {{ getSourceLabel(record.source) }}
                  </Badge>
                </TableCell>
                <TableCell class="max-w-[260px]">
                  <span class="line-clamp-2 break-all">
                    {{ record.comment || "-" }}
                  </span>
                </TableCell>
                <TableCell class="whitespace-nowrap">
                  <HumanFriendlyTime
                    :value="record.created_at"
                    :locale="locale"
                  />
                </TableCell>
                <TableCell class="whitespace-nowrap">
                  <HumanFriendlyTime
                    :value="record.updated_at"
                    :locale="locale"
                  />
                </TableCell>
                <TableCell class="text-right pr-6">
                  <ConfirmDangerPopover
                    :title="t('admin.sessions.generalBlacklist.deleteOneTitle')"
                    :description="
                      t('admin.sessions.generalBlacklist.deleteDescription')
                    "
                    :loading="isDeleting"
                    :disabled="isDeleting"
                    :on-confirm="() => deleteOne(record.ip)"
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="text-destructive"
                        :disabled="isDeleting"
                      >
                        <Trash2 class="h-4 w-4" />
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <TableSkeletonBlock
            v-else-if="showTableSkeleton"
            :header-widths="[
              'w-4',
              'w-28',
              'w-16',
              'w-24',
              'w-20',
              'w-20',
              'w-10',
            ]"
            :row-widths="[
              'w-4',
              'w-32',
              'w-16',
              'w-28',
              'w-20',
              'w-20',
              'w-10',
            ]"
          />
          <div v-else class="h-[380px]" aria-hidden="true"></div>
        </div>
      </div>

      <PagedTableFooter
        :total="totalRecords"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </div>

    <Dialog v-model:open="addDialogOpen">
      <DialogContent class="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{{
            t("admin.sessions.generalBlacklist.addDialogTitle")
          }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.sessions.generalBlacklist.addDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-4 py-2">
          <div class="grid gap-2">
            <Label for="general-blacklist-ips">
              {{ t("admin.sessions.generalBlacklist.ipInputLabel") }}
            </Label>
            <Textarea
              id="general-blacklist-ips"
              v-model="addIpsText"
              :placeholder="
                t('admin.sessions.generalBlacklist.ipInputPlaceholder')
              "
              class="min-h-[160px] font-mono text-sm"
            />
            <p class="text-xs text-muted-foreground">
              {{
                t("admin.sessions.generalBlacklist.parsedCount", {
                  count: parsedAddIps.length,
                })
              }}
            </p>
          </div>

          <div class="grid gap-2">
            <Label for="general-blacklist-comment">
              {{ t("admin.sessions.generalBlacklist.comment") }}
            </Label>
            <Textarea
              id="general-blacklist-comment"
              v-model="addComment"
              :placeholder="
                t('admin.sessions.generalBlacklist.commentPlaceholder')
              "
              class="min-h-[72px]"
            />
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isAdding"
            @click="addDialogOpen = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="parsedAddIps.length === 0 || isAdding"
            @click="addManualBlacklist"
          >
            <Loader2 v-if="isAdding" class="h-4 w-4 animate-spin" />
            <Ban v-else class="h-4 w-4" />
            {{ t("admin.sessions.generalBlacklist.addConfirm") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
