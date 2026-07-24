<script setup lang="ts">
import { computed, type UnwrapNestedRefs } from "vue";
import { useI18n } from "vue-i18n";
import { ShieldCheck } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import type { HostMapping } from "@/types";
import { useMappingVisibility } from "./useMappingVisibility";

const props = defineProps<{
  composedPreviewHost: string;
  mappingForm: HostMapping;
  visibilityEditor: UnwrapNestedRefs<ReturnType<typeof useMappingVisibility>>;
}>();

const { t } = useI18n();
const visibilityModeModel = computed({
  get: () => props.visibilityEditor.visibilityMode,
  set: (value: HostMapping["visibility"]["mode"]) => {
    props.visibilityEditor.visibilityMode = value;
  },
});
const visibilityCustomCidrsModel = computed({
  get: () => props.visibilityEditor.customCidrsText,
  set: (value: string) => {
    props.visibilityEditor.customCidrsText = value;
  },
});
</script>

<template>
  <div class="space-y-4 pb-4 pt-4">
    <div class="rounded-lg border bg-muted/20 px-4 py-2.5">
      <p class="text-xs text-muted-foreground">Host</p>
      <p class="truncate text-sm font-medium">
        {{ composedPreviewHost || mappingForm.host || "-" }}
      </p>
    </div>

    <Alert class="border-primary/25 bg-primary/5">
      <ShieldCheck class="h-4 w-4" />
      <AlertTitle>
        {{ t("admin.subdomainProxy.visibilityPriorityAlertTitle") }}
      </AlertTitle>
      <AlertDescription class="leading-6">
        {{ t("admin.subdomainProxy.visibilityPriorityAlertDescription") }}
      </AlertDescription>
    </Alert>

    <div class="space-y-2 rounded-lg border px-4 py-3">
      <div class="space-y-1">
        <Label for="mapping-visibility-mode">
          {{ t("admin.subdomainProxy.visibilityBehavior") }}
        </Label>
        <p class="text-xs leading-5 text-muted-foreground">
          {{ t("admin.subdomainProxy.visibilityBehaviorDescription") }}
        </p>
      </div>
      <Select v-model="visibilityModeModel">
        <SelectTrigger id="mapping-visibility-mode" class="w-full">
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

    <template v-if="visibilityModeModel === 'custom'">
      <section class="space-y-3 rounded-lg border px-4 py-3">
        <div class="text-sm font-medium">
          {{ t("admin.subdomainProxy.visibilityRegions") }}
        </div>
        <CidrRegionSelector
          v-model="mappingForm.visibility.selections"
          :disabled="visibilityEditor.regionInputsDisabled"
          :description="t('admin.subdomainProxy.visibilityRegionsDescription')"
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
          <Label for="mapping-visibility-cidrs">
            {{ t("admin.gatewayVisibilitySettings.customCidrs") }}
          </Label>
          <p class="text-xs leading-5 text-muted-foreground">
            {{ t("admin.subdomainProxy.visibilityCidrsDescription") }}
          </p>
        </div>
        <Textarea
          id="mapping-visibility-cidrs"
          v-model="visibilityCustomCidrsModel"
          class="min-h-28 font-mono text-sm"
          :placeholder="t('admin.gatewayVisibilitySettings.cidrPlaceholder')"
        />
        <p
          v-if="visibilityEditor.customCidrsState.invalid.length > 0"
          class="text-xs text-destructive"
        >
          {{
            t("admin.gatewayVisibilitySettings.invalidCidrs", {
              items: visibilityEditor.customCidrsState.invalid.join("、"),
            })
          }}
        </p>
        <p v-else class="text-xs text-muted-foreground">
          {{
            t("admin.gatewayVisibilitySettings.customCidrsRecognized", {
              count: visibilityEditor.customCidrsState.cidrs.length,
            })
          }}
        </p>
      </section>

      <p
        v-if="visibilityEditor.visibilityValidationMessage"
        class="rounded-lg bg-destructive/5 px-4 py-3 text-sm text-destructive"
      >
        {{ visibilityEditor.visibilityValidationMessage }}
      </p>
    </template>
  </div>
</template>
