<template>
  <div
    v-if="configStore.isLoading"
    class="ml-4 animate-pulse text-sm text-muted-foreground"
    role="status"
  >
    {{ t("common.loadingConfig") }}
  </div>
  <div
    v-if="configStore.isError"
    class="ml-4 text-sm text-destructive"
    role="alert"
  >
    {{ t("common.loadConfigFailed") }}
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const { t } = useI18n();
</script>
