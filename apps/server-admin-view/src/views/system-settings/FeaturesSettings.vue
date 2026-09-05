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
import DateTimeDisplaySettingRow from "./DateTimeDisplaySettingRow.vue";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";
import FeaturePageSettingRow from "./FeaturePageSettingRow.vue";
import { useFeaturesSettings } from "./useFeaturesSettings";

const {
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

      <button
        type="button"
        class="flex w-full items-center justify-between bg-muted/5 p-6 text-left transition-colors hover:bg-muted/15"
        @click="openSidebarMenuOrder"
      >
        <div class="min-w-0 space-y-1 pr-6">
          <div class="text-base font-medium">
            {{ t("admin.featuresSettings.sidebarMenuOrder") }}
          </div>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.featuresSettings.sidebarMenuOrderHint") }}
          </div>
        </div>
        <ChevronRight class="h-5 w-5 shrink-0 text-muted-foreground" />
      </button>

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
