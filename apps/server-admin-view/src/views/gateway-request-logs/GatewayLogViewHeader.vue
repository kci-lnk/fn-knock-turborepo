<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowLeft } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
defineProps<{
  clientIp: string;
  viewMode: string;
  selectedDate: string;
  detailRequests: number | null;
  loading: boolean;
  hasFilters: boolean;
  ipLocation: (ip: string) => string;
  setViewMode: (mode: "requests" | "ips") => Promise<void>;
  backToIps: () => Promise<void>;
  showAllIpRequests: () => Promise<void>;
}>();
const { t } = useI18n();
</script>
<template>
  <div
    v-if="!clientIp"
    class="flex w-fit items-center gap-1 rounded-lg bg-muted p-1"
    role="group"
    :aria-label="t('admin.gatewayRequestLogs.ipView.viewLabel')"
  >
    <Button
      :variant="viewMode === 'requests' ? 'secondary' : 'ghost'"
      size="sm"
      :aria-pressed="viewMode === 'requests'"
        :class="{ 'bg-background shadow-sm': viewMode === 'requests' }"
      @click="setViewMode('requests')"
      >{{ t("admin.gatewayRequestLogs.ipView.individual") }}</Button
    >
    <Button
      :variant="viewMode === 'ips' ? 'secondary' : 'ghost'"
      size="sm"
      :aria-pressed="viewMode === 'ips'"
        :class="{ 'bg-background shadow-sm': viewMode === 'ips' }"
      @click="setViewMode('ips')"
      >{{ t("admin.gatewayRequestLogs.ipView.grouped") }}</Button
    >
  </div>
  <div v-else class="space-y-2">
    <Button variant="ghost" size="sm" class="-ml-3" @click="backToIps"
      ><ArrowLeft class="mr-2 h-4 w-4" />{{
        t("admin.gatewayRequestLogs.ipView.back")
      }}</Button
    >
    <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
      <h3 class="break-all font-mono text-lg font-semibold">
        {{
          clientIp === "unknown"
            ? t("admin.gatewayRequestLogs.ipView.unknown")
            : clientIp
        }}
      </h3>
      <span class="text-sm text-muted-foreground">{{
        ipLocation(clientIp)
      }}</span>
    </div>
    <p class="text-sm text-muted-foreground">
      {{ selectedDate
      }}<template v-if="detailRequests !== null && !loading">
        ·
        {{
          t("admin.gatewayRequestLogs.ipView.detailCount", {
            count: detailRequests,
          })
        }}</template
      >
    </p>
    <Button
      v-if="hasFilters"
      variant="outline"
      size="sm"
      @click="showAllIpRequests"
      >{{ t("admin.gatewayRequestLogs.ipView.showAll") }}</Button
    >
  </div>
</template>
