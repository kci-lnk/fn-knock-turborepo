<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ShieldCheck } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import BlacklistHitsTable from "@admin-shared/components/session/BlacklistHitsTable.vue";
import type { IpBlacklistPageController } from "./useIpBlacklistPage";

const props = defineProps<{ controller: IpBlacklistPageController }>();
const { t } = useI18n();
const {
  detailHitRows,
  detailRecord,
  formatDate,
  isDetailLoading,
  isDetailsModalOpen,
  isResolvingFalsePositive,
  resolveFalsePositive,
} = props.controller;
</script>

<template>
  <DetailDialog
    v-model:open="isDetailsModalOpen"
    :title="t('admin.sessions.ipBlacklist.detailTitle')"
    :description="t('admin.sessions.ipBlacklist.detailDescription')"
    max-width-class="sm:max-w-[700px] max-w-[calc(100vw-1rem)] p-4 sm:p-6"
    :loading="isDetailLoading"
    close-variant="outline"
  >
    <div v-if="detailRecord" class="space-y-4 overflow-x-auto">
      <div class="grid gap-3 md:grid-cols-2">
        <div
          class="space-y-1 rounded-lg border p-4"
          :class="detailRecord.ipLocation ? 'md:col-span-2' : ''"
        >
          <div class="text-sm text-muted-foreground">IP</div>
          <div class="break-all font-mono text-base">
            {{ detailRecord.ip }}
          </div>
          <div
            v-if="detailRecord.ipLocation"
            class="break-all text-xs text-muted-foreground"
          >
            {{ detailRecord.ipLocation }}
          </div>
        </div>

        <div class="space-y-2 rounded-lg border p-4">
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessions.ipBlacklist.blockedAt") }}
          </div>
          <div class="break-all text-base">
            {{ formatDate(detailRecord.blockedAt) }}
          </div>
        </div>

        <div class="space-y-2 rounded-lg border p-4">
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessions.ipBlacklist.triggerWindow") }}
          </div>
          <div class="break-all text-base">
            {{
              t("admin.sessions.ipBlacklist.minutes", {
                count: detailRecord.windowMinutes,
              })
            }}
          </div>
        </div>

        <div class="space-y-2 rounded-lg border p-4">
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessions.ipBlacklist.triggerThreshold") }}
          </div>
          <div class="break-all text-base">
            {{
              t("admin.sessions.ipBlacklist.times", {
                count: detailRecord.threshold,
              })
            }}
          </div>
        </div>
      </div>

      <BlacklistHitsTable :rows="detailHitRows">
        <template #action="{ row }">
          <Button
            type="button"
            variant="outline"
            size="sm"
            :disabled="isResolvingFalsePositive"
            @click="resolveFalsePositive(row.path)"
          >
            <ShieldCheck class="mr-2 h-4 w-4" />
            {{ t("admin.sessions.ipBlacklist.allowFalsePositive") }}
          </Button>
        </template>
      </BlacklistHitsTable>
    </div>
  </DetailDialog>
</template>
