<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import TraceIdLink from "@/components/TraceIdLink.vue";
import type { GatewayLogEntry } from "@/types";
import {
  buildGatewayLogDetailCopyText,
  buildGatewayLogDetailItems,
} from "./model";
const isDetailsOpen = defineModel<boolean>("open", { required: true });
const props = defineProps<{ entry: GatewayLogEntry | null }>();
const { t, locale } = useI18n();
const detailItems = computed(() =>
  buildGatewayLogDetailItems(props.entry, t, String(locale.value)),
);
const detailCopyText = computed(() =>
  buildGatewayLogDetailCopyText(detailItems.value),
);
</script>
<template>
  <DetailDialog
    v-model:open="isDetailsOpen"
    :title="t('admin.gatewayRequestLogs.detailTitle')"
    :description="t('admin.gatewayRequestLogs.detailDescription')"
    max-width-class="sm:max-w-[640px]"
    close-variant="default"
    :copy-text="detailCopyText"
  >
    <div v-if="entry" class="space-y-4">
      <TraceIdLink :trace-id="entry.trace_id || entry.waf_trace_id" />
      <DetailFieldsGrid :items="detailItems" />
    </div>
  </DetailDialog>
</template>
