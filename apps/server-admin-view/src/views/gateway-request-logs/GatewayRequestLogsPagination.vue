<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import CursorPaginationDock from "@/components/CursorPaginationDock.vue";
import type { CursorPaginationLabels } from "@/components/cursor-pagination-contract";
import { LIMIT_OPTIONS } from "./model";

const props = defineProps<{
  canLoadNewer: boolean;
  canLoadOlder: boolean;
  cursorPageLabel: string;
  handleLimitChange: (value: unknown) => Promise<void> | void;
  handleLoadFirst: () => Promise<void> | void;
  handleLoadNewer: () => Promise<void> | void;
  handleLoadOlder: () => Promise<void> | void;
  limit: string;
  loading: boolean;
  shouldFloat: boolean;
}>();

const { t } = useI18n();
const labels = computed<CursorPaginationLabels>(() => ({
  ariaLabel: t("admin.gatewayRequestLogs.title"),
  canLoadOlder: t("admin.gatewayRequestLogs.canLoadOlder"),
  firstPage: t("admin.gatewayRequestLogs.firstPage"),
  lastPage: t("admin.gatewayRequestLogs.lastPage"),
  nextPage: t("admin.gatewayRequestLogs.nextPage"),
  pageSize: t("admin.gatewayRequestLogs.pageSize"),
  pageSizeOption: (count) =>
    t("admin.gatewayRequestLogs.pageSizeOption", { count }),
  previousPage: t("admin.gatewayRequestLogs.previousPage"),
}));
</script>

<template>
  <CursorPaginationDock
    v-bind="props"
    :labels="labels"
    :limit-options="LIMIT_OPTIONS"
  />
</template>
