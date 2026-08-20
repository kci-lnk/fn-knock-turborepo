<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import type { GatewayProxyProtocolConfig } from "@/types";
import GatewayEditorRow from "./GatewayEditorRow.vue";

defineProps<{ summary: GatewayProxyProtocolConfig | null }>();
defineEmits<{ action: [] }>();
const { t } = useI18n();
</script>

<template>
  <GatewayEditorRow
    :title="t('admin.gatewaySettings.proxyProtocol')"
    :description="t('admin.gatewaySettings.proxyProtocolDescription')"
    :action-label="t('admin.gatewaySettings.editProxyProtocol')"
    @action="$emit('action')"
  >
    <template #badges>
      <Badge
        :variant="summary?.effective_enabled ? 'default' : 'secondary'"
        class="rounded-full px-2.5"
      >
        {{
          summary?.effective_enabled
            ? t("admin.gatewaySettings.enabled")
            : t("admin.gatewaySettings.disabled")
        }}
      </Badge>
      <Badge
        v-if="summary?.managed_frp_enabled"
        variant="secondary"
        class="rounded-full px-2.5"
      >
        {{ t("admin.gatewaySettings.proxyProtocolManagedFrp") }}
      </Badge>
      <Badge
        v-if="summary?.enabled"
        variant="secondary"
        class="rounded-full px-2.5"
      >
        {{
          t("admin.gatewaySettings.proxyProtocolTrustedCount", {
            count: summary.trusted_sources.length,
          })
        }}
      </Badge>
    </template>
  </GatewayEditorRow>
</template>
