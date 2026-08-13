<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Ban, Trash2, Unlock } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";

defineProps<{
  isBlockingIps: boolean;
  isDeleting: boolean;
  isMutatingBlacklistIps: boolean;
  isReleasingIps: boolean;
  loading: boolean;
  selectedBlockedCount: number;
  selectedDate: string;
  selectedUnblockedCount: number;
}>();

const emit = defineEmits<{
  blockSelected: [];
  deleteDate: [];
  refresh: [];
  releaseSelected: [];
}>();
const { t } = useI18n();
</script>

<template>
  <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
    <div class="space-y-1">
      <div class="flex items-center gap-2">
        <h2 class="text-lg font-semibold tracking-tight">
          {{ t("admin.wafLogs.title") }}
        </h2>
        <span class="text-xs text-muted-foreground">{{ selectedDate }}</span>
      </div>
      <p class="text-sm text-muted-foreground">
        {{ t("admin.wafLogs.description") }}
      </p>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <RefreshButton
        :loading="loading"
        :disabled="loading"
        @click="emit('refresh')"
      />
      <ConfirmDangerPopover
        v-if="selectedUnblockedCount > 0"
        :title="
          t('admin.wafLogs.blacklistSelectedTitle', {
            count: selectedUnblockedCount,
          })
        "
        :description="t('admin.wafLogs.blacklistDescription')"
        :loading="isBlockingIps"
        :disabled="selectedUnblockedCount === 0 || isMutatingBlacklistIps"
        :on-confirm="() => emit('blockSelected')"
      >
        <template #trigger>
          <Button
            variant="outline"
            class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
            :disabled="selectedUnblockedCount === 0 || isMutatingBlacklistIps"
          >
            <Ban class="mr-2 h-4 w-4" />
            {{
              t("admin.wafLogs.blacklistSelected", {
                count: selectedUnblockedCount,
              })
            }}
          </Button>
        </template>
      </ConfirmDangerPopover>
      <ConfirmDangerPopover
        v-if="selectedBlockedCount > 0"
        :title="
          t('admin.wafLogs.unblacklistSelectedTitle', {
            count: selectedBlockedCount,
          })
        "
        :description="t('admin.wafLogs.unblacklistDescription')"
        :loading="isReleasingIps"
        :disabled="selectedBlockedCount === 0 || isMutatingBlacklistIps"
        :on-confirm="() => emit('releaseSelected')"
      >
        <template #trigger>
          <Button
            variant="outline"
            class="text-foreground"
            :disabled="selectedBlockedCount === 0 || isMutatingBlacklistIps"
          >
            <Unlock class="mr-2 h-4 w-4" />
            {{
              t("admin.wafLogs.unblacklistSelected", {
                count: selectedBlockedCount,
              })
            }}
          </Button>
        </template>
      </ConfirmDangerPopover>
      <ConfirmDangerPopover
        :title="t('admin.wafLogs.deleteDateTitle', { date: selectedDate })"
        :description="t('admin.wafLogs.deleteDateDescription')"
        :loading="isDeleting"
        :disabled="isDeleting"
        :on-confirm="() => emit('deleteDate')"
      >
        <template #trigger>
          <Button
            variant="outline"
            class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
            :disabled="isDeleting"
          >
            <Trash2 class="mr-2 h-4 w-4" />
            {{ t("admin.wafLogs.deleteDateAction") }}
          </Button>
        </template>
      </ConfirmDangerPopover>
    </div>
  </div>
</template>
