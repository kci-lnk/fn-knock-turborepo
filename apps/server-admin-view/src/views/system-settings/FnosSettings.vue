<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronRight } from "lucide-vue-next";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import FnosConnectWafSetting from "./fnos-settings/FnosConnectWafSetting.vue";
import { useFnosSettingsController } from "./fnos-settings/useFnosSettingsController";

const a11yId = useId();
const { t } = useI18n();
const {
  bbrCurrentDescription,
  bbrDesiredDescription,
  bbrStateMismatchDescription,
  bbrSupportDescription,
  canUseFnosCertificateSync,
  canUseFnosConnectWaf,
  canUseFnosNetworkTuning,
  certificateSyncDetails,
  form,
  iconHijackForm,
  isBbrSupported,
  isIconHijackSaving,
  isLoading,
  isMtuProbingSupported,
  isNetworkTuningAvailable,
  isNetworkTuningSaving,
  isRestrictedByRunMode,
  isSaving,
  isShareBypassMode,
  mtuCurrentDescription,
  mtuDesiredDescription,
  mtuStateMismatchDescription,
  networkTuningForm,
  networkTuningStatus,
  networkTuningUnavailableText,
  openCertificateSync,
  saveIconHijackEnabled,
  saveNetworkTuning,
  saveShareBypassEnabled,
  showLoadingSkeleton,
  toggleIconHijack,
  toggleShareBypass,
} = useFnosSettingsController();
</script>

<template>
  <Card>
    <CardContent v-if="isLoading && showLoadingSkeleton" class="p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="p-0 divide-y">
      <button
        v-if="canUseFnosCertificateSync"
        type="button"
        class="flex w-full items-center justify-between bg-muted/10 p-6 text-left transition-colors hover:bg-muted/20"
        @click="openCertificateSync"
      >
        <div class="space-y-1 pr-6">
          <div class="text-base font-medium">
            {{ t("admin.fnosCertificateSync.entryTitle") }}
          </div>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.fnosCertificateSync.entryDescription") }}
          </div>
          <div v-if="certificateSyncDetails" class="text-xs text-zinc-500">
            {{
              t("admin.fnosCertificateSync.entrySummary", {
                total: certificateSyncDetails.summary.total,
                syncable: certificateSyncDetails.summary.syncable,
              })
            }}
          </div>
        </div>
        <ChevronRight class="h-5 w-5 shrink-0 text-muted-foreground" />
      </button>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            :for="`${a11yId}-fnossettings-1`"
            class="text-base font-medium"
            :class="
              isShareBypassMode
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
            @click="toggleShareBypass"
          >
            {{ t("admin.fnosSettings.shareBypassTitle") }}
          </Label>
          <div
            class="text-sm"
            :class="
              isShareBypassMode ? 'text-muted-foreground' : 'text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.shareBypassDescription") }}
          </div>
          <div
            v-if="isRestrictedByRunMode"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ t("admin.fnosSettings.shareBypassDirectUnavailable") }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-fnossettings-1`"
          :model-value="isShareBypassMode ? form.enabled : false"
          :disabled="!isShareBypassMode || isSaving"
          @update:model-value="saveShareBypassEnabled($event === true)"
        />
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            :for="`${a11yId}-fnossettings-2`"
            class="cursor-pointer text-base font-medium"
            @click="toggleIconHijack"
          >
            {{ t("admin.fnosSettings.iconHijackTitle") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.fnosSettings.iconHijackDescriptionPrefix")
            }}<u>{{ t("admin.fnosSettings.iconHijackDescriptionHighlight") }}</u
            >{{ t("admin.fnosSettings.iconHijackDescriptionSuffix") }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-fnossettings-2`"
          :model-value="iconHijackForm.enabled"
          :disabled="isIconHijackSaving"
          @update:model-value="saveIconHijackEnabled($event === true)"
        />
      </div>

      <FnosConnectWafSetting v-if="canUseFnosConnectWaf" />

      <div
        v-if="canUseFnosNetworkTuning"
        class="flex items-center justify-between bg-muted/10 p-6"
      >
        <div class="space-y-1 pr-6">
          <Label
            :for="`${a11yId}-fnos-bbr`"
            class="text-base font-medium"
            :class="
              isNetworkTuningAvailable && isBbrSupported
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.bbrTitle") }}
          </Label>
          <div
            class="text-sm"
            :class="
              isNetworkTuningAvailable && isBbrSupported
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.bbrDescription") }}
          </div>
          <div class="text-xs leading-5 text-zinc-500">
            {{ bbrDesiredDescription }}
          </div>
          <div class="text-xs leading-5 text-zinc-500">
            {{ bbrCurrentDescription }}
          </div>
          <div
            v-if="bbrStateMismatchDescription"
            class="text-xs leading-5 text-amber-600"
          >
            {{ bbrStateMismatchDescription }}
          </div>
          <div
            v-if="networkTuningStatus"
            class="text-xs leading-5"
            :class="
              networkTuningStatus.bbr.supported
                ? 'text-emerald-600'
                : 'text-amber-600'
            "
          >
            {{ bbrSupportDescription }}
          </div>
          <div
            v-if="!isNetworkTuningAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ networkTuningUnavailableText }}
          </div>
          <div
            v-if="networkTuningStatus?.last_error"
            class="text-xs leading-5 text-destructive"
          >
            {{
              t("admin.fnosSettings.networkTuningLastError", {
                message: networkTuningStatus.last_error,
              })
            }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-fnos-bbr`"
          :model-value="networkTuningForm.bbr_enabled"
          :disabled="
            !isNetworkTuningAvailable ||
            !isBbrSupported ||
            isNetworkTuningSaving
          "
          @update:model-value="
            saveNetworkTuning(
              { bbr_enabled: $event === true },
              'admin.fnosSettings.bbrUpdated',
            )
          "
        />
      </div>

      <div
        v-if="canUseFnosNetworkTuning && isMtuProbingSupported"
        class="flex items-center justify-between bg-muted/10 p-6"
      >
        <div class="space-y-1 pr-6">
          <Label
            :for="`${a11yId}-fnos-mtu-probing`"
            class="text-base font-medium"
            :class="
              isNetworkTuningAvailable
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.mtuTitle") }}
          </Label>
          <div
            class="text-sm"
            :class="
              isNetworkTuningAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.mtuDescription") }}
          </div>
          <div class="text-xs leading-5 text-zinc-500">
            {{ mtuDesiredDescription }}
          </div>
          <div class="text-xs leading-5 text-zinc-500">
            {{ mtuCurrentDescription }}
          </div>
          <div
            v-if="mtuStateMismatchDescription"
            class="text-xs leading-5 text-amber-600"
          >
            {{ mtuStateMismatchDescription }}
          </div>
          <div
            v-if="!isNetworkTuningAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ networkTuningUnavailableText }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-fnos-mtu-probing`"
          :model-value="networkTuningForm.mtu_probing_enabled"
          :disabled="!isNetworkTuningAvailable || isNetworkTuningSaving"
          @update:model-value="
            saveNetworkTuning(
              { mtu_probing_enabled: $event === true },
              'admin.fnosSettings.mtuUpdated',
            )
          "
        />
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[160px]" aria-hidden="true" />
  </Card>
</template>
