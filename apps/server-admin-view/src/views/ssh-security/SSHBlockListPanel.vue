<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { toast } from "@admin-shared/utils/toast";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import RefreshButton from "@/components/RefreshButton.vue";
import { Loader2, Trash2 } from "lucide-vue-next";
import { SSHSecurityAPI } from "../../lib/api";
import type { SSHSecurityBlockRecord } from "../../types";

const props = defineProps<{
  reloadDetails: () => Promise<void>;
}>();

const { t } = useI18n();

const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sshSecurity.unblockFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.unblockDescription"),
      ),
    });
  },
});

const reasonLabel = (reason: SSHSecurityBlockRecord["reason"]) =>
  reason === "cidr_not_allowed"
    ? t("admin.sshSecurity.reasonRegionNotAllowed")
    : t("admin.sshSecurity.reasonThresholdReached");

const {
  items: blockRecords,
  total: blockTotal,
  loading: isLoadingBlocks,
  searchQuery: blockSearch,
  currentPage: blockPage,
  limit: blockLimit,
  parsedLimit: blockParsedLimit,
  selectedKeys: selectedBlockIps,
  isAllSelected: isAllBlocksSelected,
  fetchList: loadBlocks,
  handleSearch: handleBlockSearch,
  handlePageChange: handleBlockPageChange,
  handleLimitChange: handleBlockLimitChange,
  toggleSelect: toggleBlockSelect,
  clearSelection: clearBlockSelection,
} = usePagedSelectionList<SSHSecurityBlockRecord, string>({
  fetchPage: async ({ page, limit, query }) => {
    const payload = await SSHSecurityAPI.getBlocks(page, limit, query);
    return { items: payload.items, total: payload.total };
  },
  getKey: (record) => record.ip,
  onError: (error) => {
    toast.error(t("admin.sshSecurity.blocksLoadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.blocksLoadDescription"),
      ),
    });
  },
});

const reloadAfterChange = async () => {
  await Promise.all([loadBlocks(), props.reloadDetails()]);
};

const deleteBlocks = async (ips: string[]) => {
  if (ips.length === 0) return;
  await runDelete(() => SSHSecurityAPI.deleteBlocks(ips), {
    onSuccess: async () => {
      toast.success(t("admin.sshSecurity.unblocked"));
      clearBlockSelection();
      await reloadAfterChange();
    },
  });
};

const deleteOneBlock = async (ip: string) => {
  await runDelete(() => SSHSecurityAPI.deleteBlock(ip), {
    onSuccess: async () => {
      toast.success(t("admin.sshSecurity.unblocked"));
      selectedBlockIps.value.delete(ip);
      selectedBlockIps.value = new Set(selectedBlockIps.value);
      await reloadAfterChange();
    },
  });
};

onMounted(() => {
  void loadBlocks();
});

defineExpose({ loadBlocks });
</script>

<template>
  <div class="space-y-3">
    <div class="flex flex-wrap items-center gap-2">
      <SearchInput
        v-model="blockSearch"
        :placeholder="t('admin.sshSecurity.searchBlocksPlaceholder')"
        class="w-full max-w-xs"
        @search="handleBlockSearch"
      />
      <div class="flex-1"></div>
      <RefreshButton
        :loading="isLoadingBlocks"
        :disabled="isLoadingBlocks"
        @click="loadBlocks"
      />
      <ConfirmDangerPopover
        :title="
          t('admin.sshSecurity.confirmUnblockSelectedTitle', {
            count: selectedBlockIps.size,
          })
        "
        :description="t('admin.sshSecurity.unblockDescriptionText')"
        :loading="isDeleting"
        :disabled="selectedBlockIps.size === 0 || isDeleting"
        :on-confirm="() => deleteBlocks(Array.from(selectedBlockIps))"
      >
        <template #trigger>
          <Button
            variant="destructive"
            :disabled="selectedBlockIps.size === 0 || isDeleting"
          >
            <Trash2 class="h-4 w-4" />
            {{
              t("admin.sshSecurity.deleteSelected", {
                count: selectedBlockIps.size,
              })
            }}
          </Button>
        </template>
      </ConfirmDangerPopover>
    </div>

    <Card class="border-border/60 shadow-none">
      <CardContent class="p-0">
        <div class="overflow-auto">
          <Table class="min-w-[860px]">
            <TableHeader>
              <TableRow>
                <TableHead class="h-11 w-[48px] px-3">
                  <Checkbox
                    v-model="isAllBlocksSelected"
                    :aria-label="t('common.selectAll')"
                  />
                </TableHead>
                <TableHead class="h-11 min-w-[220px] px-4">
                  {{ t("admin.sshSecurity.ipLocation") }}
                </TableHead>
                <TableHead class="h-11 w-[168px] px-4">
                  {{ t("admin.sshSecurity.blockedAt") }}
                </TableHead>
                <TableHead class="h-11 w-[168px] px-4">
                  {{ t("admin.sshSecurity.expiresAt") }}
                </TableHead>
                <TableHead class="h-11 w-[120px] px-4">
                  {{ t("admin.sshSecurity.reason") }}
                </TableHead>
                <TableHead class="h-11 w-[120px] px-4">
                  {{ t("admin.sshSecurity.count") }}
                </TableHead>
                <TableHead class="h-11 w-[88px] px-4 text-right">
                  {{ t("admin.sshSecurity.actions") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="isLoadingBlocks">
                <TableCell colspan="7" class="px-4 py-10 text-center">
                  <Loader2
                    class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                  />
                </TableCell>
              </TableRow>
              <TableRow v-else-if="blockRecords.length === 0">
                <TableCell
                  colspan="7"
                  class="px-4 py-10 text-center text-muted-foreground"
                >
                  {{ t("admin.sshSecurity.noBlockRecords") }}
                </TableCell>
              </TableRow>
              <TableRow v-for="record in blockRecords" v-else :key="record.ip">
                <TableCell class="px-3 py-3 align-top">
                  <Checkbox
                    :model-value="selectedBlockIps.has(record.ip)"
                    :aria-label="t('common.selectItem', { item: record.ip })"
                    @update:model-value="toggleBlockSelect(record.ip)"
                  />
                </TableCell>
                <TableCell
                  class="min-w-[220px] px-4 py-3 align-top whitespace-normal"
                >
                  <div class="font-mono text-sm">{{ record.ip }}</div>
                  <div
                    v-if="record.ipLocation"
                    class="mt-0.5 text-xs text-muted-foreground"
                  >
                    {{ record.ipLocation }}
                  </div>
                </TableCell>
                <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                  <HumanFriendlyTime :value="record.blocked_at" />
                </TableCell>
                <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                  <HumanFriendlyTime :value="record.expires_at" />
                </TableCell>
                <TableCell class="px-4 py-3 align-top">
                  <Badge :variant="record.applied ? 'secondary' : 'outline'">
                    {{ reasonLabel(record.reason) }}
                  </Badge>
                </TableCell>
                <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                  {{ record.failed_count }} / {{ record.threshold }}
                </TableCell>
                <TableCell class="px-4 py-3 text-right align-top">
                  <ConfirmDangerPopover
                    :title="t('admin.sshSecurity.confirmUnblockOneTitle')"
                    :description="t('admin.sshSecurity.unblockDescriptionText')"
                    :loading="isDeleting"
                    :disabled="isDeleting"
                    :on-confirm="() => deleteOneBlock(record.ip)"
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="icon"
                        :aria-label="t('common.confirmDelete')"
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
        </div>
        <PagedTableFooter
          :total="blockTotal"
          :page="blockPage"
          :limit="blockLimit"
          :items-per-page="blockParsedLimit"
          @update:page="handleBlockPageChange"
          @update:limit="handleBlockLimitChange"
        />
      </CardContent>
    </Card>
  </div>
</template>
