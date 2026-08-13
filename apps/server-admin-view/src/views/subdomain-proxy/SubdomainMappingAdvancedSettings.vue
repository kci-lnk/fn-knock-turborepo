<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { SubdomainMappingDialogProps } from "./subdomain-mapping-dialog-contract";
import SubdomainMappingAccessSettings from "./SubdomainMappingAccessSettings.vue";
import SubdomainMappingProxyProtocolSettings from "./SubdomainMappingProxyProtocolSettings.vue";
import SubdomainMappingVisibilityEntry from "./SubdomainMappingVisibilityEntry.vue";

defineProps<{ dialog: SubdomainMappingDialogProps }>();
const { t } = useI18n();
</script>

<template>
  <div class="space-y-3 pt-2">
    <h3 class="text-sm font-semibold">
      {{ t("admin.subdomainProxy.advancedConfig") }}
    </h3>
    <SubdomainMappingAccessSettings :dialog="dialog" />
    <SubdomainMappingProxyProtocolSettings :dialog="dialog" />
    <SubdomainMappingVisibilityEntry :dialog="dialog" />
  </div>
</template>
