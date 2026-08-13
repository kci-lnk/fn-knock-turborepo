<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
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
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { RefreshCw, ShieldCheck, Trash2 } from "lucide-vue-next";
import type { IpWhitelistPageController } from "./useIpWhitelistPage";

const props = defineProps<{ controller: IpWhitelistPageController }>();
const { t } = useI18n();
const {
  currentPage,
  fetchRecords,
  filteredRecords,
  formatRemaining,
  getResolveStatusLabel,
  getResolveStatusVariant,
  handleLimitChange,
  handlePageChange,
  isInitializing,
  limit,
  loading,
  paginatedRecords,
  parsedLimit,
  records,
  refreshingId,
  refreshRecord,
  removeRecord,
  removingId,
  saveComment,
  searchQuery,
  showInitializingSkeleton,
  targetTypeBadgeLabel,
} = props.controller;
</script>

<template>
  <div v-if="!isInitializing" class="mb-4 flex items-center space-x-2">
    <SearchInput
      v-model="searchQuery"
      :placeholder="t('admin.ipWhitelist.searchPlaceholder')"
      class="max-w-xs"
    />
    <RefreshButton
      icon-only
      :loading="loading"
      :disabled="loading"
      @click="fetchRecords"
    />
  </div>
  <div
    v-else-if="showInitializingSkeleton"
    class="mb-4 flex items-center space-x-2"
  >
    <Skeleton class="h-9 w-60" />
    <Skeleton class="h-9 w-9 rounded-md" />
  </div>
  <div v-else class="mb-4 h-9" aria-hidden="true" />

  <div v-if="!isInitializing" class="rounded-md border">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t("admin.ipWhitelist.target") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.status") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.source") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.createdAt") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.comment") }}</TableHead>
          <TableHead class="w-[180px] text-right">
            {{ t("admin.ipWhitelist.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="loading && records.length === 0">
          <TableCell
            colspan="6"
            class="py-6 text-center text-muted-foreground"
          >
            {{ t("admin.ipWhitelist.loading") }}
          </TableCell>
        </TableRow>
        <TableRow v-else-if="paginatedRecords.length === 0">
          <TableCell
            colspan="6"
            class="py-6 text-center text-muted-foreground"
          >
            {{ t("admin.ipWhitelist.empty") }}
          </TableCell>
        </TableRow>
        <TableRow v-for="record in paginatedRecords" :key="record.id">
          <TableCell class="font-medium">
            <div class="flex items-center gap-2">
              <span>{{ record.ip }}</span>
              <Badge variant="outline">
                {{ targetTypeBadgeLabel(record.targetType) }}
              </Badge>
            </div>
            <div v-if="record.targetType === 'cname'" class="mt-2 space-y-1">
              <div
                v-for="resolvedTarget in record.resolvedTargets || []"
                :key="resolvedTarget"
              >
                <Badge variant="secondary" class="font-normal">
                  {{ resolvedTarget }}
                </Badge>
              </div>
              <span
                v-if="!(record.resolvedTargets || []).length"
                class="block text-xs text-muted-foreground"
              >
                {{ t("admin.ipWhitelist.noResolvedRecords") }}
              </span>
            </div>
            <div
              v-if="record.targetType === 'cname' && record.resolveMessage"
              class="mt-1 text-xs text-muted-foreground"
            >
              {{ record.resolveMessage }}
            </div>
            <div
              v-if="record.ipLocation"
              class="mt-0.5 text-xs text-muted-foreground"
            >
              {{ record.ipLocation }}
            </div>
          </TableCell>
          <TableCell>
            <template v-if="record.targetType === 'cname'">
              <div class="flex flex-col items-start gap-1.5">
                <Badge :variant="getResolveStatusVariant(record)">
                  {{ getResolveStatusLabel(record) }}
                </Badge>
                <span class="text-xs text-muted-foreground">
                  {{
                    t("admin.ipWhitelist.checkInterval", {
                      minutes: record.checkIntervalMinutes || 5,
                    })
                  }}
                </span>
                <span
                  v-if="record.lastCheckedAt"
                  class="text-xs text-muted-foreground"
                >
                  {{ t("admin.ipWhitelist.lastCheckedAt") }}
                  <HumanFriendlyTime :value="record.lastCheckedAt * 1000" />
                </span>
                <span
                  v-if="record.expireAt"
                  class="text-xs text-muted-foreground"
                >
                  {{ t("admin.ipWhitelist.expiresAt") }}
                  <HumanFriendlyTime :value="record.expireAt * 1000" />
                </span>
                <div v-else class="flex items-center text-sm text-green-600">
                  <ShieldCheck class="mr-1 h-4 w-4" />
                  {{ t("admin.ipWhitelist.permanent") }}
                </div>
              </div>
            </template>
            <template v-else>
              <div
                v-if="!record.expireAt"
                class="flex items-center text-green-600"
              >
                <ShieldCheck class="mr-1 h-4 w-4" />
                {{ t("admin.ipWhitelist.permanent") }}
              </div>
              <div v-else class="flex flex-col">
                <span>{{ formatRemaining(record.expireAt) }}</span>
                <span class="text-xs text-muted-foreground">
                  {{ t("admin.ipWhitelist.expiresAt") }}
                  <HumanFriendlyTime :value="record.expireAt * 1000" />
                </span>
              </div>
            </template>
          </TableCell>
          <TableCell>
            <Badge
              :variant="record.source === 'manual' ? 'default' : 'secondary'"
            >
              {{
                record.source === "manual"
                  ? t("admin.ipWhitelist.sourceManual")
                  : t("admin.ipWhitelist.sourceLoginGrant")
              }}
            </Badge>
          </TableCell>
          <TableCell class="whitespace-nowrap text-xs text-muted-foreground">
            <HumanFriendlyTime :value="record.createdAt * 1000" />
          </TableCell>
          <TableCell>
            <InlineCommentEditor
              :text="record.comment"
              :save="(value) => saveComment(record.id, value)"
            />
          </TableCell>
          <TableCell class="text-right">
            <div class="flex justify-end gap-2">
              <Button
                v-if="record.targetType === 'cname'"
                variant="outline"
                size="sm"
                :disabled="refreshingId === record.id"
                @click="refreshRecord(record.id)"
              >
                <RefreshCw
                  :class="[
                    'mr-1 h-4 w-4',
                    refreshingId === record.id ? 'animate-spin' : '',
                  ]"
                />
                {{ t("admin.ipWhitelist.refreshNow") }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.ipWhitelist.deleteTitle')"
                :description="
                  t('admin.ipWhitelist.deleteDescription', {
                    target: record.ip,
                  })
                "
                :loading="removingId === record.id"
                :disabled="removingId === record.id"
                :on-confirm="() => removeRecord(record.id)"
                content-class="w-60 text-left"
              >
                <template #trigger>
                  <Button
                    variant="ghost"
                    size="icon"
                    :aria-label="t('common.confirmDelete')"
                    class="h-8 w-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
                    :disabled="removingId === record.id"
                  >
                    <Trash2 class="h-4 w-4" />
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
  <div
    v-else-if="showInitializingSkeleton"
    class="rounded-md border"
  >
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t("admin.ipWhitelist.target") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.statusExpires") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.source") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.createdAt") }}</TableHead>
          <TableHead>{{ t("admin.ipWhitelist.comment") }}</TableHead>
          <TableHead class="w-[180px] text-right">
            {{ t("admin.ipWhitelist.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="index in 6" :key="index">
          <TableCell><Skeleton class="h-4 w-40" /></TableCell>
          <TableCell><Skeleton class="h-4 w-24" /></TableCell>
          <TableCell><Skeleton class="h-4 w-14" /></TableCell>
          <TableCell><Skeleton class="h-4 w-28" /></TableCell>
          <TableCell><Skeleton class="h-4 w-32" /></TableCell>
          <TableCell class="text-right">
            <Skeleton class="ml-auto h-8 w-20 rounded-md" />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
  <div v-else class="h-[320px]" aria-hidden="true" />

  <PagedTableFooter
    class="mt-4 rounded-md border"
    :total="filteredRecords.length"
    :page="currentPage"
    :limit="limit"
    :items-per-page="parsedLimit"
    @update:page="handlePageChange"
    @update:limit="handleLimitChange"
  />
</template>
