<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import type { GatewayVisibilitySelection, HostVisibilityMode } from "@/types";

defineProps<{
  behaviorDescription: string;
  regionsDescription: string;
  cidrsDescription: string;
  validationMessage: string;
}>();

const mode = defineModel<HostVisibilityMode>("mode", { required: true });
const selections = defineModel<GatewayVisibilitySelection[]>("selections", {
  required: true,
});
const customCidrsText = defineModel<string>("customCidrsText", {
  required: true,
});
const { t } = useI18n();
const id = useId();
const customCidrsState = computed(() =>
  parseCidrTextarea(customCidrsText.value),
);
</script>

<template>
  <div class="space-y-4">
    <div class="space-y-2 rounded-lg border px-4 py-3">
      <div class="space-y-1">
        <Label :for="`${id}-mode`">
          {{ t("admin.subdomainProxy.visibilityBehavior") }}
        </Label>
        <p class="text-xs leading-5 text-muted-foreground">
          {{ behaviorDescription }}
        </p>
      </div>
      <Select v-model="mode">
        <SelectTrigger :id="`${id}-mode`" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="inherit">
            {{ t("admin.subdomainProxy.visibilityInherit") }}
          </SelectItem>
          <SelectItem value="custom">
            {{ t("admin.subdomainProxy.visibilityCustom") }}
          </SelectItem>
          <SelectItem value="disabled">
            {{ t("admin.subdomainProxy.visibilityDisabled") }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <template v-if="mode === 'custom'">
      <section class="space-y-3 rounded-lg border px-4 py-3">
        <div class="text-sm font-medium">
          {{ t("admin.subdomainProxy.visibilityRegions") }}
        </div>
        <CidrRegionSelector
          v-model="selections"
          :description="regionsDescription"
          :text="{
            add: t('admin.gatewayVisibilitySettings.saveSelection'),
            addRegion: t('admin.gatewayVisibilitySettings.manageRegions'),
            cancel: t('admin.subdomainProxy.cancel'),
            dialogDescription: t(
              'admin.gatewayVisibilitySettings.addRegionDescription',
            ),
            loadFailed: t('admin.gatewayVisibilitySettings.cityLoadFailed'),
            loadFailedDescription: t(
              'admin.subdomainProxy.visibilityRegionsLoadFailed',
            ),
            loading: t('admin.gatewayVisibilitySettings.loading'),
            noRegions: t('admin.gatewayVisibilitySettings.noRegions'),
            province: t('admin.gatewayVisibilitySettings.province'),
            retry: t('admin.subdomainProxy.retry'),
            selectedCount: (count) =>
              t('admin.gatewayVisibilitySettings.selectedRegionCount', {
                count,
              }),
            scope: t('admin.gatewayVisibilitySettings.scope'),
            selectCity: t('admin.gatewayVisibilitySettings.selectCity'),
            selectProvince: t('admin.gatewayVisibilitySettings.selectProvince'),
            selectProvinceFirst: t(
              'admin.gatewayVisibilitySettings.selectProvinceFirst',
            ),
            unavailable: t(
              'admin.gatewayVisibilitySettings.unavailableSelection',
            ),
          }"
        />
      </section>

      <section class="space-y-3 rounded-lg border px-4 py-3">
        <div class="space-y-1">
          <Label :for="`${id}-cidrs`">
            {{ t("admin.gatewayVisibilitySettings.customCidrs") }}
          </Label>
          <p class="text-xs leading-5 text-muted-foreground">
            {{ cidrsDescription }}
          </p>
        </div>
        <Textarea
          :id="`${id}-cidrs`"
          v-model="customCidrsText"
          class="min-h-28 font-mono text-sm"
          :placeholder="t('admin.gatewayVisibilitySettings.cidrPlaceholder')"
        />
        <p
          v-if="customCidrsState.invalid.length > 0"
          class="text-xs text-destructive"
        >
          {{
            t("admin.gatewayVisibilitySettings.invalidCidrs", {
              items: customCidrsState.invalid.join("、"),
            })
          }}
        </p>
        <p v-else class="text-xs text-muted-foreground">
          {{
            t("admin.gatewayVisibilitySettings.customCidrsRecognized", {
              count: customCidrsState.cidrs.length,
            })
          }}
        </p>
      </section>

      <p
        v-if="validationMessage"
        role="alert"
        class="rounded-lg bg-destructive/5 px-4 py-3 text-sm text-destructive"
      >
        {{ validationMessage }}
      </p>
    </template>
  </div>
</template>
