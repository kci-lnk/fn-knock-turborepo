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
const versionOptions = computed(() => [
  { value: "v1", label: t("admin.gatewayPortalSettings.versionV1") },
  { value: "v2", label: t("admin.gatewayPortalSettings.versionV2") },
]);
const displayOptions = computed(() => [
  {
    value: "domain",
    label: t("admin.gatewayPortalSettings.displayDomain"),
  },
  {
    value: "title",
    label: t("admin.gatewayPortalSettings.displayTitle"),
  },
]);
const dragModeOptions = computed(() => [
  {
    value: "corners",
    label: t("admin.gatewayPortalSettings.iconDragModeCorners"),
  },
  {
    value: "free",
    label: t("admin.gatewayPortalSettings.iconDragModeFree"),
  },
]);
</script>

<template>
  <section class="p-6">
    <div class="rounded-2xl border border-border/60 bg-muted/10 px-4 py-4">
      <div class="flex items-start justify-between gap-4">
        <div class="min-w-0 space-y-1.5">
          <Label :for="`${a11yId}-enabled`" class="text-base font-medium">
            {{ t("admin.gatewayPortalSettings.enabled") }}
          </Label>
          <div class="text-sm leading-6 text-muted-foreground">
            {{ t("admin.gatewayPortalSettings.enabledDescription") }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-enabled`"
          class="mt-0.5 shrink-0"
          :model-value="model.form.enabled"
          :disabled="model.isSaving"
          @update:model-value="model.saveEnabled($event === true)"
        />
      </div>
    </div>
  </section>

  <template v-if="model.form.enabled">
    <GatewayPortalChoiceSetting
      :title="t('admin.gatewayPortalSettings.version')"
      :description="t('admin.gatewayPortalSettings.versionDescription')"
      :model-value="model.form.version"
      :options="versionOptions"
      :disabled="model.isSaving"
      @update:model-value="model.saveVersion($event === 'v2' ? 'v2' : 'v1')"
    />
    <GatewayPortalChoiceSetting
      :title="t('admin.gatewayPortalSettings.display')"
      :description="t('admin.gatewayPortalSettings.displayDescription')"
      :model-value="model.form.display_style"
      :options="displayOptions"
      :disabled="model.isSaving"
      @update:model-value="
        model.saveDisplayStyle($event === 'domain' ? 'domain' : 'title')
      "
    />
    <GatewayPortalChoiceSetting
      :title="t('admin.gatewayPortalSettings.iconDragMode')"
      :description="t('admin.gatewayPortalSettings.iconDragModeDescription')"
      :model-value="model.form.icon_drag_mode"
      :options="dragModeOptions"
      :disabled="model.isSaving"
      @update:model-value="
        model.saveIconDragMode($event === 'free' ? 'free' : 'corners')
      "
    />

    <section class="flex items-center justify-between gap-4 p-6">
      <div class="space-y-1 pr-6">
        <Label :for="`${a11yId}-app-icon`" class="text-base">
          {{ t("admin.gatewayPortalSettings.showAppIcon") }}
        </Label>
        <div class="text-sm text-muted-foreground">
          {{ t("admin.gatewayPortalSettings.showAppIconDescription") }}
        </div>
      </div>
      <Switch
        :id="`${a11yId}-app-icon`"
        :model-value="model.form.show_app_icon"
        :disabled="model.isSaving"
        @update:model-value="model.saveShowAppIcon($event === true)"
      />
    </section>

    <section
      v-if="model.wolFeatureEnabled"
      class="flex items-center justify-between gap-4 p-6"
    >
      <div class="space-y-1 pr-6">
        <Label :for="`${a11yId}-wol`" class="text-base">
          {{ t("admin.gatewayPortalSettings.showWol") }}
        </Label>
        <div class="text-sm text-muted-foreground">
          {{ t("admin.gatewayPortalSettings.showWolDescription") }}
        </div>
      </div>
      <Switch
        :id="`${a11yId}-wol`"
        :model-value="model.form.show_wol"
        :disabled="model.isSaving"
        @update:model-value="model.saveShowWOL($event === true)"
      />
    </section>
  </template>
</template>
