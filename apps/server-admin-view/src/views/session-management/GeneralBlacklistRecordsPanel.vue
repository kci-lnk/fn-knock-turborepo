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
import { Loader2, Plus, Trash2 } from "lucide-vue-next";
import type { GeneralBlacklistPageController } from "./useGeneralBlacklistPage";

const props = defineProps<{ controller: GeneralBlacklistPageController }>();
const { t, locale } = useI18n();
const {
  addDialogOpen,
  currentPage,
  deleteBlacklist,
  deleteOne,
  fetchBlacklist,
  getLocationText,
  getSourceLabel,
  getSourceVariant,
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
} = props.controller;
</script>

<template>
  <div class="flex flex-col gap-3 lg:flex-row lg:items-center">
    <SearchInput
      v-model="searchQuery"
      :placeholder="t('admin.sessions.generalBlacklist.searchPlaceholder')"
      class="w-full lg:w-[280px]"
      @search="handleSearch"
    />
    <div class="flex-1" />
    <div class="flex flex-wrap items-center gap-2">
      <RefreshButton
        :loading="loading"
        :disabled="loading"
        @click="fetchBlacklist"
      />
      <Button @click="addDialogOpen = true">
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
                {{ t("admin.sessions.generalBlacklist.ipLocationHeader") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.generalBlacklist.source") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.generalBlacklist.comment") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.generalBlacklist.createdAt") }}
              </TableHead>
              <TableHead>
                {{ t("admin.sessions.generalBlacklist.updatedAt") }}
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
                {{ t("admin.sessions.generalBlacklist.empty") }}
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
                <div class="break-all font-mono text-sm">{{ record.ip }}</div>
                <div class="mt-0.5 break-all text-xs text-muted-foreground">
                  {{ getLocationText(record.ip) }}
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
                <HumanFriendlyTime :value="record.created_at" :locale="locale" />
              </TableCell>
              <TableCell class="whitespace-nowrap">
                <HumanFriendlyTime :value="record.updated_at" :locale="locale" />
              </TableCell>
              <TableCell class="pr-6 text-right">
                <ConfirmDangerPopover
                  :title="t('admin.sessions.generalBlacklist.deleteOneTitle')"
                  :description="t('admin.sessions.generalBlacklist.deleteDescription')"
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
