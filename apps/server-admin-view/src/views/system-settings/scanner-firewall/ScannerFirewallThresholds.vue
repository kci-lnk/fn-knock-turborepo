<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { ScannerFirewallSettingsModel } from "./useScannerFirewallSettings";

defineProps<{ model: ScannerFirewallSettingsModel; idPrefix: string }>();
const { t } = useI18n();
</script>

<template>
  <div
    class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center"
  >
    <div class="space-y-1 pr-6">
      <Label :for="`${idPrefix}-window`" class="text-base">
        {{ t("admin.scannerFirewallSettings.windowTitle") }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.windowDescription") }}
        <span
          v-if="model.derivedWindowMinutes > model.form.windowMinutes"
          class="block text-destructive sm:ml-1 sm:inline"
        >
          {{
            t("admin.scannerFirewallSettings.enforcedMinimum", {
              minutes: model.baseWindowMinutes,
            })
          }}
        </span>
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Input
        :id="`${idPrefix}-window`"
        v-model.number="model.form.windowMinutes"
        type="number"
        min="1"
        class="w-24 text-center"
      />
      <span class="w-12 text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.minutesUnit") }}
      </span>
    </div>
  </div>

  <div
    class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center"
  >
    <div class="space-y-1 pr-6">
      <Label :for="`${idPrefix}-threshold`" class="text-base">
        {{ t("admin.scannerFirewallSettings.thresholdTitle") }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.thresholdDescription") }}
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Input
        :id="`${idPrefix}-threshold`"
        v-model.number="model.form.threshold"
        type="number"
        min="1"
        class="w-24 text-center"
      />
      <span class="w-12 text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.timesUnit") }}
      </span>
    </div>
  </div>

  <div
    class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center"
  >
    <div class="space-y-1 pr-6">
      <Label :for="`${idPrefix}-ttl`" class="text-base">
        {{ t("admin.scannerFirewallSettings.blacklistTtlTitle") }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.blacklistTtlDescription") }}
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Input
        :id="`${idPrefix}-ttl`"
        v-model.number="model.form.blacklistTtlDays"
        type="number"
        min="1"
        class="w-24 text-center"
      />
      <span class="w-12 text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.daysUnit") }}
      </span>
    </div>
  </div>
</template>
