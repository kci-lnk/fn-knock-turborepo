<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Plus, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { ScannerPathWhitelistSettingsModel } from "./useScannerPathWhitelistSettings";

defineProps<{ model: ScannerPathWhitelistSettingsModel }>();
const { t } = useI18n();
</script>

<template>
  <div class="space-y-4">
    <div
      v-if="model.entries.length === 0"
      class="rounded-xl border border-dashed px-5 py-10 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.scannerPathWhitelist.empty") }}
    </div>

    <div
      v-else
      class="divide-y overflow-hidden rounded-xl border bg-background"
    >
      <div
        v-for="(entry, index) in model.entries"
        :key="entry.id"
        class="space-y-2 px-4 py-3"
      >
        <div class="flex items-center gap-3">
          <Label :for="`scanner-path-${entry.id}`" class="sr-only">
            {{
              t("admin.scannerPathWhitelist.pathLabel", { index: index + 1 })
            }}
          </Label>
          <Input
            :id="`scanner-path-${entry.id}`"
            :model-value="entry.value"
            class="font-mono text-sm"
            :placeholder="t('admin.scannerPathWhitelist.pathPlaceholder')"
            :aria-invalid="Boolean(model.entryErrors[entry.id])"
            :disabled="model.isSaving"
            @update:model-value="model.setEntryPath(entry.id, String($event))"
          />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            class="shrink-0 text-destructive"
            :aria-label="t('admin.scannerPathWhitelist.removePath')"
            :disabled="model.isSaving"
            @click="model.removeEntry(entry.id)"
          >
            <Trash2 class="h-4 w-4" />
          </Button>
        </div>
        <p
          v-if="model.entryErrors[entry.id]"
          class="text-xs text-destructive"
          role="alert"
        >
          {{ model.entryErrors[entry.id] }}
        </p>
      </div>
    </div>

    <Button
      type="button"
      variant="outline"
      :disabled="model.isSaving"
      @click="model.addEntry"
    >
      <Plus class="mr-2 h-4 w-4" />
      {{ t("admin.scannerPathWhitelist.addPath") }}
    </Button>
  </div>
</template>
