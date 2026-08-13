<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import CursorPaginationDock from "@/components/CursorPaginationDock.vue";
import type { CursorPaginationLabels } from "@/components/cursor-pagination-contract";

const LIMIT_OPTIONS = ["20", "50", "100", "200"] as const;
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
  ariaLabel: t("admin.wafLogs.title"),
  canLoadOlder: t("admin.wafLogs.canLoadOlder"),
  firstPage: t("admin.wafLogs.firstPage"),
  lastPage: t("admin.wafLogs.lastPage"),
  nextPage: t("admin.wafLogs.nextPage"),
  pageSize: t("admin.wafLogs.pageSize"),
  pageSizeOption: (count) => t("admin.wafLogs.pageSizeOption", { count }),
  previousPage: t("admin.wafLogs.previousPage"),
}));
</script>

<template>
  <CursorPaginationDock
    v-bind="props"
    :labels="labels"
    :limit-options="LIMIT_OPTIONS"
  />
</template>
