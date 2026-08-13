<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { SubdomainMappingDialogProps } from "./subdomain-mapping-dialog-contract";

const { dialog } = defineProps<{ dialog: SubdomainMappingDialogProps }>();
const { t } = useI18n();
const sendProxyHeadersModel = computed({
  get: () => dialog.sendProxyHeaders,
  set: (value: boolean) => dialog.setSendProxyHeaders(value),
});
const preserveHostModel = computed({
  get: () => dialog.preserveHost,
  set: (value: boolean) => dialog.setPreserveHost(value),
});
const protocolModeModel = computed({
  get: () => dialog.mappingForm.protocol_mode || "auto",
  set: (value) =>
    dialog.updateMappingForm({
      protocol_mode: value === "http1" || value === "http2" ? value : "auto",
    }),
});
const mappingWafEnabledModel = computed({
  get: () => dialog.mappingForm.waf_enabled !== false,
  set: (value: boolean) => dialog.updateMappingForm({ waf_enabled: value }),
});
</script>

<template>
  <div
    class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
  >
    <div class="min-w-0 space-y-1">
      <Label for="mapping-proxy-headers">
        {{ t("admin.subdomainProxy.proxyHeaders") }}
      </Label>
      <p class="text-xs leading-5 text-muted-foreground">
        {{
          dialog.gatewayProxyHeadersBlockedReason ||
          t("admin.subdomainProxy.proxyHeadersDescription")
        }}
      </p>
    </div>
    <Switch
      id="mapping-proxy-headers"
      v-model="sendProxyHeadersModel"
      :disabled="
        dialog.isSavingMappings || !!dialog.gatewayProxyHeadersBlockedReason
      "
    />
  </div>

  <div
    class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
  >
    <div class="min-w-0 space-y-1">
      <Label for="mapping-host-response">
        {{ t("admin.subdomainProxy.hostResponse") }}
      </Label>
      <p class="text-xs leading-5 text-muted-foreground">
        {{
          dialog.gatewayHostResponseBlockedReason ||
          t("admin.subdomainProxy.hostResponseDescription")
        }}
      </p>
    </div>
    <Switch
      id="mapping-host-response"
      v-model="preserveHostModel"
      :disabled="
        dialog.isSavingMappings || !!dialog.gatewayHostResponseBlockedReason
      "
    />
  </div>

  <div class="space-y-2 rounded-lg border px-4 py-3">
    <div class="space-y-1">
      <Label for="mapping-protocol-mode">
        {{ t("admin.subdomainProxy.protocolMode") }}
      </Label>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.subdomainProxy.protocolModeDescription") }}
      </p>
    </div>
    <Select v-model="protocolModeModel" :disabled="dialog.isSavingMappings">
      <SelectTrigger
        id="mapping-protocol-mode"
        class="w-full"
        :disabled="dialog.isSavingMappings"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="auto">
          {{ t("admin.subdomainProxy.protocolModeAuto") }}
        </SelectItem>
        <SelectItem value="http1">
          {{ t("admin.subdomainProxy.protocolModeHttp1") }}
        </SelectItem>
        <SelectItem value="http2">
          {{ t("admin.subdomainProxy.protocolModeHttp2") }}
        </SelectItem>
      </SelectContent>
    </Select>
  </div>

  <div
    v-if="dialog.globalWafEnabled && !dialog.isMappingAuthService"
    class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
  >
    <div class="min-w-0 space-y-1">
      <Label for="mapping-waf">
        {{ t("admin.subdomainProxy.wafEnabled") }}
      </Label>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.subdomainProxy.wafEnabledDescription") }}
      </p>
    </div>
    <Switch
      id="mapping-waf"
      v-model="mappingWafEnabledModel"
      :disabled="dialog.isSavingMappings"
    />
  </div>
</template>
