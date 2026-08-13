<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { StaleHostMappingsCleanupDialogModel } from "./useStaleHostMappingsCleanupDialog";

defineProps<{ model: StaleHostMappingsCleanupDialogModel }>();
const { t } = useI18n();
</script>

<template>
  <div class="hidden rounded-md border bg-background sm:block">
    <Table class="w-full table-fixed" container-class="overflow-hidden">
      <colgroup>
        <col class="w-[6%]" />
        <col class="w-[21%]" />
        <col class="w-[29%]" />
        <col class="w-[25%]" />
        <col class="w-[19%]" />
      </colgroup>
      <TableHeader
        class="sticky top-0 z-10 bg-background shadow-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-background"
      >
        <TableRow class="h-12">
          <TableHead class="px-4 text-center">
            <input
              type="checkbox"
              :aria-label="t('common.selectAll')"
              class="h-4 w-4 cursor-pointer"
              :checked="model.isAllStaleSelected"
              :disabled="model.staleResults.length === 0"
              @change="model.handleToggleAllStale"
            />
          </TableHead>
          <TableHead class="px-4">
            {{ t("admin.subdomainProxy.staleCleanupColumns.title") }}
          </TableHead>
          <TableHead class="px-4">
            {{ t("admin.subdomainProxy.staleCleanupColumns.host") }}
          </TableHead>
          <TableHead class="px-4">
            {{ t("admin.subdomainProxy.staleCleanupColumns.target") }}
          </TableHead>
          <TableHead class="px-4">
            {{ t("admin.subdomainProxy.staleCleanupColumns.status") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow
          v-for="result in model.visibleResults"
          :key="result.host"
          class="h-16"
        >
          <TableCell class="px-4 py-3 text-center align-top">
            <input
              type="checkbox"
              :aria-label="t('common.selectItem', { item: result.host })"
              class="h-4 w-4 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
              :checked="model.isHostSelected(result.host)"
              :disabled="result.status !== 'stale'"
              @change="model.handleToggleHost(result.host, $event)"
            />
          </TableCell>
          <TableCell
            class="whitespace-normal break-words px-4 py-4 text-sm font-medium leading-6 align-top"
          >
            {{ model.getMappingTitle(result.host) }}
          </TableCell>
          <TableCell
            class="whitespace-normal break-all px-4 py-4 font-medium leading-6 align-top"
          >
            {{ result.host }}
          </TableCell>
          <TableCell
            class="whitespace-normal break-all px-4 py-4 text-sm leading-6 align-top"
          >
            {{ result.target }}
          </TableCell>
          <TableCell class="whitespace-normal px-4 py-4 align-top">
            <div class="space-y-1">
              <Badge
                :variant="model.getStatusBadgeVariant(result.status)"
                :class="model.getStatusBadgeClass(result.status)"
              >
                {{ model.getStatusLabel(result) }}
              </Badge>
              <p
                v-if="result.error"
                class="whitespace-normal break-words text-xs leading-5 text-muted-foreground"
              >
                {{ result.error }}
              </p>
            </div>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>

  <div class="space-y-3 sm:hidden">
    <label
      class="flex items-center gap-3 rounded-md border bg-background px-4 py-3 text-sm font-medium"
    >
      <input
        type="checkbox"
        class="h-4 w-4 cursor-pointer"
        :checked="model.isAllStaleSelected"
        :disabled="model.staleResults.length === 0"
        @change="model.handleToggleAllStale"
      />
      {{ t("admin.subdomainProxy.staleCleanupSelectAll") }}
    </label>

    <div
      v-for="result in model.visibleResults"
      :key="`mobile-${result.host}`"
      class="rounded-md border bg-background p-4"
    >
      <div class="flex items-start gap-3">
        <input
          type="checkbox"
          :aria-label="t('common.selectItem', { item: result.host })"
          class="mt-1 h-4 w-4 shrink-0 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
          :checked="model.isHostSelected(result.host)"
          :disabled="result.status !== 'stale'"
          @change="model.handleToggleHost(result.host, $event)"
        />
        <div class="min-w-0 flex-1 space-y-3">
          <div class="min-w-0">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.subdomainProxy.staleCleanupColumns.title") }}
            </p>
            <p class="break-words text-sm font-medium leading-6">
              {{ model.getMappingTitle(result.host) }}
            </p>
          </div>
          <div class="min-w-0">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.subdomainProxy.staleCleanupColumns.host") }}
            </p>
            <p class="break-all text-sm font-medium leading-6">
              {{ result.host }}
            </p>
          </div>
          <div class="min-w-0">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.subdomainProxy.staleCleanupColumns.target") }}
            </p>
            <p class="break-all text-sm leading-6">{{ result.target }}</p>
          </div>
          <div class="min-w-0 space-y-1">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.subdomainProxy.staleCleanupColumns.status") }}
            </p>
            <Badge
              :variant="model.getStatusBadgeVariant(result.status)"
              :class="model.getStatusBadgeClass(result.status)"
            >
              {{ model.getStatusLabel(result) }}
            </Badge>
            <p
              v-if="result.error"
              class="break-words text-xs leading-5 text-muted-foreground"
            >
              {{ result.error }}
            </p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
