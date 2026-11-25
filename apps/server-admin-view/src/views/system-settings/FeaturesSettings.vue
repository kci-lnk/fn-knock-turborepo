<script setup lang="ts">
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import DateTimeDisplaySettingRow from "./DateTimeDisplaySettingRow.vue";
import IframeEmbeddingSettingRow from "./IframeEmbeddingSettingRow.vue";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";
import FeaturePageSettingRow from "./FeaturePageSettingRow.vue";
import { useFeaturesSettings } from "./useFeaturesSettings";

const {
  allowIframeEmbedding,
  autoHttpsEnabled,
  autoHttpsRuntimeError,
  dateTimeDisplayMode,
  isDashboardDisplaySwitchDisabled,
  isLoading,
  isProtocolMappingAvailable,
  isSaving,
  isSmartConnectAvailable,
  isSSHSecurityAvailable,
  openSmartConnect,
  openWebTerminal,
  openSidebarMenuOrder,
  passkeyBindPromptEnabled,
  protocolMappingDisabledReason,
  protocolMappingEnabled,
  saveAutoHttpsEnabled,
  saveDateTimeDisplayMode,
  saveAllowIframeEmbedding,
  savePasskeyBindPromptEnabled,
  saveProtocolMappingEnabled,
  saveShowConsoleAppList,
  saveShowEntryStatusModule,
  saveSSHSecurityEnabled,
  saveWOLEnabled,
  showAutoHttpsEntry,
  showConsoleAppList,
  showConsoleAppListEntry,
  showEntryStatusModule,
  showLoadingSkeleton,
  showSmartConnectEntry,
  showSSHSecurityEntry,
  smartConnectDisabledReason,
  sshSecurityDisabledReason,
  sshSecurityEnabled,
  t,
  wolEnabled,
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
        v-if="showConsoleAppListEntry"
        :title="t('admin.featuresSettings.showConsoleAppList')"
        :description="t('admin.featuresSettings.showConsoleAppListHint')"
        :model-value="showConsoleAppList"
        :disabled="isDashboardDisplaySwitchDisabled"
        @change="saveShowConsoleAppList"
      />

      <DateTimeDisplaySettingRow
        :model-value="dateTimeDisplayMode"
        :disabled="isDashboardDisplaySwitchDisabled"
        @change="saveDateTimeDisplayMode"
      />
      <IframeEmbeddingSettingRow
        :model-value="allowIframeEmbedding"
        :disabled="isDashboardDisplaySwitchDisabled"
        @change="saveAllowIframeEmbedding"
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
        :title="t('admin.featuresSettings.wol')"
        :description="t('admin.featuresSettings.wolHint')"
        :model-value="wolEnabled"
        :disabled="isSaving"
        @change="saveWOLEnabled"
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

      <FeaturePageSettingRow
        :title="t('admin.featuresSettings.sidebarMenuOrder')"
        :description="t('admin.featuresSettings.sidebarMenuOrderHint')"
        :available="true"
        disabled-reason=""
        @open="openSidebarMenuOrder"
      />

      <FeaturePageSettingRow
        :title="t('admin.nav.webTerminal')"
        :description="t('admin.webTerminalSettings.description')"
        :available="true"
        disabled-reason=""
        @open="openWebTerminal"
      />

      <FeaturePageSettingRow
        v-if="showSmartConnectEntry"
        :title="t('admin.featuresSettings.smartConnect')"
        :description="t('admin.featuresSettings.smartConnectHint')"
        :available="isSmartConnectAvailable"
        :disabled-reason="smartConnectDisabledReason"
        @open="openSmartConnect"
      />
    </CardContent>
  </Card>
</template>
