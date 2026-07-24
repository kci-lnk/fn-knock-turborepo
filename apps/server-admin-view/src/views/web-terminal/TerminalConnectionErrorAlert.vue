<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { AlertTriangle } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

defineProps<{
  message: string;
}>();

const { t } = useI18n();
</script>

<template>
  <Alert
    v-if="message"
    variant="destructive"
    class="shrink-0 border-destructive/40"
  >
    <AlertTriangle class="h-4 w-4" />
    <AlertTitle>
      {{ t("admin.webTerminal.connectionErrorTitle") }}
    </AlertTitle>
    <AlertDescription>{{ message }}</AlertDescription>
  </Alert>
</template>
