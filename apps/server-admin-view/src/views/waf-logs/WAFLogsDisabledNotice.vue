<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Settings, ShieldAlert } from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

const emit = defineEmits<{ openSettings: [] }>();
const { t } = useI18n();
</script>

<template>
  <Alert
    class="flex items-center gap-3 rounded-lg border-dashed bg-muted/20 px-4 py-3 text-foreground shadow-none"
  >
    <ShieldAlert class="h-4 w-4 shrink-0 text-muted-foreground" />
    <div
      class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
    >
      <p class="text-sm text-muted-foreground">
        {{ t("admin.wafLogs.disabledNotice") }}
      </p>
      <Button variant="ghost" class="shrink-0" @click="emit('openSettings')">
        <Settings class="mr-2 h-4 w-4" />
        {{ t("admin.wafLogs.goSettings") }}
      </Button>
    </div>
  </Alert>
</template>
