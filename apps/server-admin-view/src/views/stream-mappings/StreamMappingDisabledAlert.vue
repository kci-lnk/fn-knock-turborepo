<script setup lang="ts">
import { TriangleAlert } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

const { t } = useI18n();
</script>

<template>
  <Alert
    class="items-start rounded-xl border-amber-200 bg-amber-50/80 text-amber-950 shadow-none"
  >
    <TriangleAlert class="mt-0.5 h-4 w-4 shrink-0" />
    <div class="space-y-1">
      <AlertTitle>{{ t("admin.streamMappings.disabledTitle") }}</AlertTitle>
      <AlertDescription class="text-sm leading-6 text-amber-900">
        {{ t("admin.streamMappings.disabledDescription") }}
      </AlertDescription>
    </div>
  </Alert>
</template>
