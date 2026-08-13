<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Loader2, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { MaintenanceClearDataController } from "./maintenance-settings-contract";

const props = defineProps<{ controller: MaintenanceClearDataController }>();
const { t } = useI18n();
const {
  canClearAllData,
  clearAllData,
  clearDataConfirmation,
  expectedClearDataConfirmation,
  handleClearDataDialogOpenChange,
  handleClearDataEnter,
  isClearDataDialogOpen,
  isClearingData,
  openClearDataDialog,
} = props.controller;
</script>

<template>
<section class="mt-6 overflow-hidden rounded-2xl border bg-background">
  <div class="border-b px-6 py-5 sm:px-8">
    <h2 class="text-xl font-semibold tracking-tight">
      {{ t("admin.maintenanceSettings.dangerZoneTitle") }}
    </h2>
    <p class="mt-1 text-sm text-muted-foreground">
      {{ t("admin.maintenanceSettings.dangerZoneDescription") }}
    </p>
  </div>

  <div
    class="flex flex-col gap-4 px-6 py-5 sm:px-8 lg:flex-row lg:items-center lg:justify-between"
  >
    <div class="space-y-1">
      <p class="text-sm font-medium">
        {{ t("admin.maintenanceSettings.clearAllDataTitle") }}
      </p>
      <p class="max-w-3xl text-sm leading-6 text-muted-foreground">
        {{ t("admin.maintenanceSettings.clearAllDataDescription") }}
      </p>
    </div>

    <Button
      variant="outline"
      class="shrink-0 border-destructive/40 text-destructive hover:bg-destructive/5 hover:text-destructive focus-visible:ring-destructive/20 lg:min-w-[168px]"
      :disabled="isClearingData"
      @click="openClearDataDialog"
    >
      <Trash2 class="mr-2 h-4 w-4" />
      {{ t("admin.maintenanceSettings.clearAllDataAction") }}
    </Button>
  </div>
</section>

<Dialog
  :open="isClearDataDialogOpen"
  @update:open="handleClearDataDialogOpenChange"
>
  <DialogContent
    class="sm:max-w-[420px]"
    :show-close-button="!isClearingData"
  >
    <DialogHeader>
      <DialogTitle class="text-left">
        {{ t("admin.maintenanceSettings.clearAllDataDialogTitle") }}
      </DialogTitle>
      <DialogDescription class="text-left text-sm leading-6">
        {{ t("admin.maintenanceSettings.clearAllDataDialogDescription") }}
      </DialogDescription>
    </DialogHeader>

    <p class="text-sm leading-6 text-destructive">
      {{ t("admin.maintenanceSettings.clearAllDataWarning") }}
    </p>

    <div class="space-y-2">
      <label for="clear-all-data-confirmation" class="text-sm font-medium">
        {{
          t("admin.maintenanceSettings.clearAllDataTypePrompt", {
            phrase: expectedClearDataConfirmation,
          })
        }}
      </label>
      <Input
        id="clear-all-data-confirmation"
        v-model="clearDataConfirmation"
        :placeholder="expectedClearDataConfirmation"
        :disabled="isClearingData"
        :aria-invalid="
          clearDataConfirmation.length > 0 &&
          clearDataConfirmation !== expectedClearDataConfirmation
            ? 'true'
            : undefined
        "
        @keydown.enter="handleClearDataEnter"
      />
    </div>

    <DialogFooter class="mt-1 gap-2">
      <Button
        variant="outline"
        :disabled="isClearingData"
        @click="handleClearDataDialogOpenChange(false)"
      >
        {{ t("common.cancel") }}
      </Button>
      <Button
        variant="destructive"
        :disabled="!canClearAllData"
        @click="clearAllData"
      >
        <Loader2 v-if="isClearingData" class="mr-2 h-4 w-4 animate-spin" />
        {{
          isClearingData
            ? t("admin.maintenanceSettings.clearingAllData")
            : t("admin.maintenanceSettings.confirmClearAllData")
        }}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
</template>
