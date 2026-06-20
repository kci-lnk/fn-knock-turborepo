<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { AlertTriangle } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

defineProps<{
  blockedReason: string;
  terminalEnabled: boolean;
}>();

const emit = defineEmits<{
  goSettings: [];
}>();

const { t } = useI18n();
</script>

<template>
  <Alert v-if="!terminalEnabled" class="border-border/60">
    <AlertTriangle class="h-4 w-4" />
    <AlertTitle>{{ t("admin.webTerminal.disabledTitle") }}</AlertTitle>
    <AlertDescription class="space-y-3">
      <p>{{ t("admin.webTerminal.disabledDescription") }}</p>
      <Button size="sm" @click="emit('goSettings')">
        {{ t("admin.webTerminal.goSettings") }}
      </Button>
    </AlertDescription>
  </Alert>

  <Alert v-else-if="blockedReason" variant="destructive" class="border-destructive/40">
    <AlertTriangle class="h-4 w-4" />
    <AlertTitle>{{ t("admin.webTerminal.unavailableTitle") }}</AlertTitle>
    <AlertDescription>{{ blockedReason }}</AlertDescription>
  </Alert>
</template>
