<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import GatewayEditorRow from "./GatewayEditorRow.vue";

defineProps<{
  enabled: boolean;
  showAppIcon: boolean;
  enabledLabel: string;
  displayLabel: string;
  versionLabel: string;
  iconLabel: string;
}>();
defineEmits<{ action: [] }>();
const { t } = useI18n();
</script>

<template>
  <GatewayEditorRow
    :title="t('admin.gatewaySettings.portal')"
    :description="t('admin.gatewaySettings.portalDescription')"
    :action-label="t('admin.gatewaySettings.editPortal')"
    @action="$emit('action')"
  >
    <template #badges>
      <Badge
        :variant="enabled ? 'default' : 'secondary'"
        class="rounded-full px-2.5"
      >
        {{ enabledLabel }}
      </Badge>
      <Badge variant="secondary" class="rounded-full px-2.5">
        {{ displayLabel }}
      </Badge>
      <Badge variant="secondary" class="rounded-full px-2.5">
        {{ versionLabel }}
      </Badge>
      <Badge
        :variant="showAppIcon ? 'default' : 'secondary'"
        class="rounded-full px-2.5"
      >
        {{ t("admin.gatewaySettings.portalIconSummary", { state: iconLabel }) }}
      </Badge>
    </template>
  </GatewayEditorRow>
</template>
