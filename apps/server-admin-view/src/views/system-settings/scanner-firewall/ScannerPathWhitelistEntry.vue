<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ChevronRight } from "lucide-vue-next";

defineEmits<{ open: [] }>();
const { t } = useI18n();
</script>

<template>
  <button
    type="button"
    class="flex w-full items-center justify-between bg-muted/5 p-6 text-left transition-colors hover:bg-muted/15"
    @click="$emit('open')"
  >
    <div class="min-w-0 space-y-1 pr-6">
      <div class="text-base font-medium">
        {{ t("admin.scannerPathWhitelist.entryTitle") }}
      </div>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.scannerPathWhitelist.entryDescription") }}
      </div>
    </div>
    <ChevronRight class="h-5 w-5 shrink-0 text-muted-foreground" />
  </button>
</template>
