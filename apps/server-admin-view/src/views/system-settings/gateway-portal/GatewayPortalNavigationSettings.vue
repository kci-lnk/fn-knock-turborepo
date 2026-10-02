<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import GatewayPortalChoiceSetting from "./GatewayPortalChoiceSetting.vue";
import type { GatewayPortalSettingsModel } from "./useGatewayPortalSettings";

defineProps<{ model: GatewayPortalSettingsModel }>();
const { t } = useI18n();
const a11yId = useId();
const navigationOptions = computed(() => [
  {
    value: "internet",
    label: t("admin.gatewayPortalSettings.navigationInternet"),
  },
  { value: "lan", label: t("admin.gatewayPortalSettings.navigationLan") },
]);
</script>

<template>
  <GatewayPortalChoiceSetting
    :title="t('admin.gatewayPortalSettings.navigationMode')"
    :description="t('admin.gatewayPortalSettings.navigationModeDescription')"
    :model-value="model.form.navigation_mode"
    :options="navigationOptions"
    :disabled="model.isSaving || model.form.smart_lan_detection"
    @update:model-value="
      model.saveNavigationMode($event === 'lan' ? 'lan' : 'internet')
    "
  />
  <section class="flex items-center justify-between gap-4 p-6">
    <div class="space-y-1 pr-6">
      <Label :for="`${a11yId}-smart-lan`" class="text-base">
        {{ t("admin.gatewayPortalSettings.smartLanDetection") }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.gatewayPortalSettings.smartLanDetectionDescription") }}
      </div>
    </div>
    <Switch
      :id="`${a11yId}-smart-lan`"
      :model-value="model.form.smart_lan_detection"
      :disabled="model.isSaving"
      @update:model-value="model.saveSmartLanDetection($event === true)"
    />
  </section>
</template>
