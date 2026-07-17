<script setup lang="ts">
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { ChevronRight } from "lucide-vue-next";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";
import { useFeaturesSettings } from "./useFeaturesSettings";

const {
  autoHttpsEnabled,
  autoHttpsRuntimeError,
  isDashboardDisplaySwitchDisabled,
  isLoading,
  isProtocolMappingAvailable,
  isSaving,
  isSmartConnectAvailable,
  isSSHSecurityAvailable,
  openSmartConnect,
  passkeyBindPromptEnabled,
  protocolMappingDisabledReason,
  protocolMappingEnabled,
  saveAutoHttpsEnabled,
  savePasskeyBindPromptEnabled,
  saveProtocolMappingEnabled,
  saveShowEntryStatusModule,
  saveSSHSecurityEnabled,
  showAutoHttpsEntry,
  showEntryStatusModule,
  showLoadingSkeleton,
  showSmartConnectEntry,
  showSSHSecurityEntry,
  smartConnectDisabledReason,
  sshSecurityDisabledReason,
  sshSecurityEnabled,
  t,
} = useFeaturesSettings();
</script>

<template>
  <Card>
    <CardHeader>
      <div class="space-y-1.5">
        <CardTitle class="text-md">{{
          t("admin.featuresSettings.title")
        }}</CardTitle>
        <CardDescription>{{
          t("admin.featuresSettings.description")
        }}</CardDescription>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <FeatureSwitchRow
        :title="t('admin.featuresSettings.showEntryStatusModule')"
        :description="t('admin.featuresSettings.showEntryStatusModuleHint')"
        :model-value="showEntryStatusModule"
        :disabled="isDashboardDisplaySwitchDisabled"
        @change="saveShowEntryStatusModule"
      />

      <FeatureSwitchRow
        :title="t('admin.featuresSettings.passkeyBindPrompt')"
        :description="t('admin.featuresSettings.passkeyBindPromptHint')"
        :model-value="passkeyBindPromptEnabled"
        :disabled="isSaving"
        @change="savePasskeyBindPromptEnabled"
      />

      <FeatureSwitchRow
        v-if="showAutoHttpsEntry"
        :title="t('admin.featuresSettings.autoHttps')"
        :description="t('admin.featuresSettings.autoHttpsHint')"
        :model-value="autoHttpsEnabled"
        :disabled="isSaving"
        :error="autoHttpsRuntimeError"
        @change="saveAutoHttpsEnabled"
      />

      <FeatureSwitchRow
        v-if="showSSHSecurityEntry"
        :title="t('admin.featuresSettings.sshSecurity')"
        :description="t('admin.featuresSettings.sshSecurityHint')"
        :model-value="sshSecurityEnabled"
        :available="isSSHSecurityAvailable"
        :disabled="isSaving"
        :disabled-reason="sshSecurityDisabledReason"
        @change="saveSSHSecurityEnabled"
      />

      <FeatureSwitchRow
        :title="t('admin.featuresSettings.protocolMapping')"
        :description="t('admin.featuresSettings.protocolMappingHint')"
        :model-value="protocolMappingEnabled"
        :available="isProtocolMappingAvailable"
        :disabled="isSaving"
        :disabled-reason="protocolMappingDisabledReason"
        @change="saveProtocolMappingEnabled"
      />

      <button
        v-if="showSmartConnectEntry"
        type="button"
        class="flex w-full items-center justify-between p-6 text-left transition-colors"
        :class="
          isSmartConnectAvailable
            ? 'bg-muted/5 hover:bg-muted/15'
            : 'cursor-not-allowed bg-muted/5'
        "
        :disabled="!isSmartConnectAvailable"
        @click="openSmartConnect"
      >
        <div class="space-y-1 pr-6">
          <div
            class="text-base font-medium"
            :class="
              isSmartConnectAvailable ? 'text-foreground' : 'text-zinc-500'
            "
          >
            {{ t("admin.featuresSettings.smartConnect") }}
          </div>
          <div
            class="text-sm"
            :class="
              isSmartConnectAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.featuresSettings.smartConnectHint") }}
          </div>
          <div
            v-if="!isSmartConnectAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ smartConnectDisabledReason }}
          </div>
        </div>
        <ChevronRight
          class="h-5 w-5 shrink-0"
          :class="
            isSmartConnectAvailable ? 'text-muted-foreground' : 'text-zinc-400'
          "
        />
      </button>
    </CardContent>
  </Card>
</template>
