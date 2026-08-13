<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Ban, Trash2, Unlock } from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";

defineProps<{
  blockIps: (ips: string[]) => Promise<void> | void;
  deleteSelectedDate: () => Promise<void> | void;
  isBlocking: boolean;
  isDeleting: boolean;
  isMutating: boolean;
  isReleasing: boolean;
  loading: boolean;
  refresh: () => Promise<void> | void;
  releaseIps: (ips: string[]) => Promise<void> | void;
  selectedBlockedIps: string[];
  selectedDate: string;
  selectedUnblockedIps: string[];
}>();
const { t } = useI18n();
</script>

<template>
  <div class="flex w-full flex-wrap items-center justify-end gap-2">
    <RefreshButton
      :loading="loading"
      :disabled="loading"
      class="px-2.5 [&_span]:hidden [&_svg]:mr-0 sm:px-3 sm:[&_span]:inline sm:[&_svg]:mr-1.5"
      @click="refresh"
    />
    <ConfirmDangerPopover
      v-if="selectedUnblockedIps.length > 0"
      :title="
        t('admin.gatewayRequestLogs.blacklistSelectedTitle', {
          count: selectedUnblockedIps.length,
        })
      "
      :description="t('admin.gatewayRequestLogs.blacklistDescription')"
      :loading="isBlocking"
      :disabled="selectedUnblockedIps.length === 0 || isMutating"
      :on-confirm="() => blockIps(selectedUnblockedIps)"
    >
      <template #trigger>
        <Button
          variant="outline"
          class="border-destructive/30 px-2.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive sm:px-4 sm:text-sm"
          :disabled="selectedUnblockedIps.length === 0 || isMutating"
        >
          <Ban class="mr-2 h-4 w-4" />
          {{
            t("admin.gatewayRequestLogs.blacklistSelected", {
              count: selectedUnblockedIps.length,
            })
          }}
        </Button>
      </template>
    </ConfirmDangerPopover>
    <ConfirmDangerPopover
      v-if="selectedBlockedIps.length > 0"
      :title="
        t('admin.gatewayRequestLogs.unblacklistSelectedTitle', {
          count: selectedBlockedIps.length,
        })
      "
      :description="t('admin.gatewayRequestLogs.unblacklistDescription')"
      :loading="isReleasing"
      :disabled="selectedBlockedIps.length === 0 || isMutating"
      :on-confirm="() => releaseIps(selectedBlockedIps)"
    >
      <template #trigger>
        <Button
          variant="outline"
          class="px-2.5 text-xs text-foreground sm:px-4 sm:text-sm"
          :disabled="selectedBlockedIps.length === 0 || isMutating"
        >
          <Unlock class="mr-2 h-4 w-4" />
          {{
            t("admin.gatewayRequestLogs.unblacklistSelected", {
              count: selectedBlockedIps.length,
            })
          }}
        </Button>
      </template>
    </ConfirmDangerPopover>
    <ConfirmDangerPopover
      :title="
        t('admin.gatewayRequestLogs.deleteDateTitle', { date: selectedDate })
      "
      :description="t('admin.gatewayRequestLogs.deleteDateDescription')"
      :loading="isDeleting"
      :disabled="isDeleting"
      :on-confirm="deleteSelectedDate"
    >
      <template #trigger>
        <Button
          variant="outline"
          class="border-destructive/30 px-2.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive sm:px-4 sm:text-sm"
          :disabled="isDeleting"
        >
          <Trash2 class="mr-2 h-4 w-4" />
          {{ t("admin.gatewayRequestLogs.deleteDateAction") }}
        </Button>
      </template>
    </ConfirmDangerPopover>
  </div>
</template>
