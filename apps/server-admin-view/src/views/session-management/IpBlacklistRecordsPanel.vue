<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import { Eye, Loader2, Settings, Trash2 } from "lucide-vue-next";
import type { IpBlacklistPageController } from "./useIpBlacklistPage";

const props = defineProps<{ controller: IpBlacklistPageController }>();
const { t, locale } = useI18n();
const {
  currentPage,
  deleteBlacklist,
  deleteOne,
  fetchBlacklist,
  goToFirewallSettings,
  handleLimitChange,
  handlePageChange,
  handleSearch,
  isAllSelected,
  isDeleting,
  limit,
  loading,
  parsedLimit,
  records,
  searchQuery,
  selectedIps,
  showTableSkeleton,
  toggleSelect,
  totalRecords,
  viewDetails,
} = props.controller;
</script>

<template>
  <div class="flex items-center gap-2">
    <SearchInput
      v-model="searchQuery"
      :placeholder="t('admin.sessions.ipBlacklist.searchPlaceholder')"
      class="w-[260px]"
      @search="handleSearch"
    />
    <div class="flex-1" />
    <RefreshButton
      :loading="loading"
      :disabled="loading"
      @click="fetchBlacklist"
    />
    <Button variant="outline" @click="goToFirewallSettings">
      <Settings class="h-4" />
      {{ t("admin.sessions.ipBlacklist.settings") }}
    </Button>
    <ConfirmDangerPopover
      :title="
        t('admin.sessions.ipBlacklist.deleteSelectedTitle', {
          count: selectedIps.size,
        })
      "
      :description="t('admin.sessions.ipBlacklist.deleteDescription')"
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
            t("admin.sessions.ipBlacklist.deleteSelected", {
              count: selectedIps.size,
            })
          }}
        </Button>
      </template>
    </ConfirmDangerPopover>
  </div>

  <div
    class="flex flex-1 flex-col overflow-hidden rounded-md border bg-background"
  >
    <div class="w-full flex-1 overflow-hidden">
      <div class="h-full overflow-auto">
        <Table v-if="!(loading && records.length === 0)">
          <TableHeader class="sticky top-0 z-10 bg-background shadow-sm">
            <TableRow>
              <TableHead class="w-[50px]">
                <Checkbox
                  v-model="isAllSelected"
                  :aria-label="t('common.selectAll')"
                />
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.ipBlacklist.ipLocationHeader") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.ipBlacklist.blockedAt") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.ipBlacklist.window") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.ipBlacklist.threshold") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.ipBlacklist.hits") }}
              </TableHead>
              <TableHead class="pr-6 text-right">
                {{ t("admin.sessions.table.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading">
              <TableCell colspan="7" class="py-10 text-center">
                <Loader2
                  class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                />
              </TableCell>
            </TableRow>
            <TableRow v-else-if="records.length === 0">
              <TableCell
                colspan="7"
                class="py-10 text-center text-muted-foreground"
              >
                {{ t("admin.sessions.ipBlacklist.empty") }}
              </TableCell>
            </TableRow>
            <TableRow v-for="record in records" v-else :key="record.ip">
              <TableCell>
                <Checkbox
                  :model-value="selectedIps.has(record.ip)"
                  :aria-label="t('common.selectItem', { item: record.ip })"
                  @update:model-value="toggleSelect(record.ip)"
                />
              </TableCell>
              <TableCell class="font-medium">
                <div class="font-mono text-sm">{{ record.ip }}</div>
                <div
                  v-if="record.ipLocation"
                  class="mt-0.5 break-all text-xs text-muted-foreground"
                >
                  {{ record.ipLocation }}
                </div>
              </TableCell>
              <TableCell class="whitespace-nowrap">
                <HumanFriendlyTime
                  :value="record.blockedAt"
                  :locale="locale"
                />
              </TableCell>
              <TableCell>
                {{
                  t("admin.sessions.ipBlacklist.minutes", {
                    count: record.windowMinutes,
                  })
                }}
              </TableCell>
              <TableCell>
                {{
                  t("admin.sessions.ipBlacklist.times", {
                    count: record.threshold,
                  })
                }}
              </TableCell>
              <TableCell>
                <Badge variant="secondary">{{ record.hits?.length || 0 }}</Badge>
              </TableCell>
              <TableCell class="space-x-2 pr-6 text-right">
                <Button
                  variant="ghost"
                  size="icon"
                  :aria-label="t('common.viewDetails')"
                  @click="viewDetails(record)"
                >
                  <Eye class="h-4 w-4" />
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.sessions.ipBlacklist.deleteOneTitle')"
                  :description="t('admin.sessions.ipBlacklist.deleteDescription')"
                  :loading="isDeleting"
                  :disabled="isDeleting"
                  :on-confirm="() => deleteOne(record.ip)"
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
        <TableSkeletonBlock
          v-else-if="showTableSkeleton"
          :header-widths="[
            'w-4',
            'w-24',
            'w-20',
            'w-10',
            'w-10',
            'w-10',
            'w-10',
          ]"
          :row-widths="[
            'w-4',
            'w-24',
            'w-20',
            'w-10',
            'w-10',
            'w-10',
            'w-16',
          ]"
        />
        <div v-else class="h-[380px]" aria-hidden="true" />
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
</template>
