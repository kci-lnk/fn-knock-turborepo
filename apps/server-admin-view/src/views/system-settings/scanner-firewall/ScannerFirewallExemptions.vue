<script setup lang="ts">
import { useI18n } from "vue-i18n";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { ScannerFirewallSettingsModel } from "./useScannerFirewallSettings";

defineProps<{ model: ScannerFirewallSettingsModel }>();
const { t } = useI18n();
</script>

<template>
  <div class="flex flex-col gap-4 p-6">
    <div class="text-base font-medium">
      {{ t("admin.scannerFirewallSettings.cidrExemptionRegionsTitle") }}
    </div>
    <CidrRegionSelector
      v-model="model.form.cidrExemptionRegions"
      :disabled="model.regionInputsDisabled"
      :description="
        t('admin.scannerFirewallSettings.cidrExemptionRegionsDescription')
      "
      :text="{
        add: t('admin.gatewayVisibilitySettings.saveSelection'),
        addRegion: t('admin.gatewayVisibilitySettings.manageRegions'),
        cancel: t('common.cancel'),
        dialogDescription: t(
          'admin.scannerFirewallSettings.addRegionDescription',
        ),
        loadFailed: t('admin.scannerFirewallSettings.regionsLoadFailed'),
        loadFailedDescription: t(
          'admin.scannerFirewallSettings.regionsLoadDescription',
        ),
        loading: t('admin.scannerFirewallSettings.loading'),
        noRegions: t('admin.scannerFirewallSettings.noRegions'),
        province: t('admin.scannerFirewallSettings.province'),
        retry: t('admin.subdomainProxy.retry'),
        selectedCount: (count) =>
          t('admin.gatewayVisibilitySettings.selectedRegionCount', { count }),
        scope: t('admin.scannerFirewallSettings.scope'),
        selectCity: t('admin.scannerFirewallSettings.selectCity'),
        selectProvince: t('admin.scannerFirewallSettings.selectProvince'),
        selectProvinceFirst: t(
          'admin.scannerFirewallSettings.selectProvinceFirst',
        ),
        unavailable: t('admin.gatewayVisibilitySettings.unavailableSelection'),
      }"
    />
  </div>

  <div class="flex flex-col gap-4 p-6">
    <div class="space-y-1">
      <Label for="scanner-cidr-exemptions" class="text-base">
        {{ t("admin.scannerFirewallSettings.cidrExemptionsTitle") }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.scannerFirewallSettings.cidrExemptionsDescription") }}
      </div>
    </div>
    <div class="w-full space-y-2">
      <Textarea
        id="scanner-cidr-exemptions"
        v-model="model.form.cidrExemptionsText"
        class="min-h-32 font-mono text-sm"
        :placeholder="t('admin.scannerFirewallSettings.cidrExemptionsPlaceholder')"
        :disabled="model.isSaving"
      />
      <div class="flex flex-wrap gap-x-4 gap-y-2 text-sm">
        <span class="text-muted-foreground">
          {{
            t("admin.scannerFirewallSettings.cidrExemptionsRecognized", {
              count: model.cidrExemptionsState.cidrs.length,
            })
          }}
        </span>
        <span
          v-if="model.invalidCidrExemptions.length > 0"
          class="text-destructive"
        >
          {{
            t("admin.scannerFirewallSettings.cidrExemptionsInvalid", {
              items: model.invalidCidrExemptions.join("、"),
            })
          }}
        </span>
        <span v-else class="text-emerald-600">
          {{ t("admin.scannerFirewallSettings.cidrExemptionsValid") }}
        </span>
      </div>
    </div>
  </div>
</template>
