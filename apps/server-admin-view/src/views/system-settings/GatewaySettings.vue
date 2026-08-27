<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";
import GatewayEditorRow from "./GatewayEditorRow.vue";
import GatewayNumberSettingRow from "./GatewayNumberSettingRow.vue";
import GatewayPortalSummaryRow from "./GatewayPortalSummaryRow.vue";
import GatewayProxyProtocolSummaryRow from "./GatewayProxyProtocolSummaryRow.vue";
import GatewayUnmatchedRouteSettingRow from "./GatewayUnmatchedRouteSettingRow.vue";
import GatewayUpstreamErrorSettingRow from "./GatewayUpstreamErrorSettingRow.vue";
import { useGatewaySettingsController } from "./useGatewaySettingsController";

const { t } = useI18n();
const {
  authCacheFailHint,
  authCacheHint,
  form,
  hostResponseDisabledReason,
  isDirty,
  isGatewaySettingsBusy,
  isHostResponseAvailable,
  isLoading,
  isProxyHeadersAvailable,
  openHostResponseEditor,
  openPortalEditor,
  openProxyHeadersEditor,
  openProxyProtocolEditor,
  openVisibilityEditor,
  portalDisplaySummary,
  portalEnabledSummary,
  portalIconSummary,
  portalSummary,
  portalVersionSummary,
  proxyHeadersDisabledReason,
  proxyProtocolSummary,
  resetForm,
  saveSettings,
  showLoadingSkeleton,
  visibilitySummary,
} = useGatewaySettingsController();
</script>

<template>
  <Card>
    <CardHeader>
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-1.5">
          <CardTitle class="text-md">{{
            t("admin.gatewaySettings.title")
          }}</CardTitle>
          <CardDescription>
            {{ t("admin.gatewaySettings.description") }}
          </CardDescription>
        </div>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <GatewayNumberSettingRow
        v-model="form.auth_cache_ttl_seconds"
        :title="t('admin.gatewaySettings.authCacheTitle')"
        :unit-label="t('admin.gatewaySettings.seconds')"
        unit-width-class="w-12"
        :min="0"
        :disabled="isGatewaySettingsBusy"
        :summary="authCacheHint"
      >
        <template #description>
          {{ t("admin.gatewaySettings.authCacheDescriptionBefore") }}
          <code>0</code>
          {{ t("admin.gatewaySettings.authCacheDescriptionAfter") }}
        </template>
      </GatewayNumberSettingRow>

      <GatewayNumberSettingRow
        v-model="form.auth_cache_unauthorized_ttl_seconds"
        :title="t('admin.gatewaySettings.authFailCacheTitle')"
        :unit-label="t('admin.gatewaySettings.seconds')"
        unit-width-class="w-12"
        :min="0"
        :disabled="isGatewaySettingsBusy"
        :summary="authCacheFailHint"
      >
        <template #description>
          {{ t("admin.gatewaySettings.authFailCacheDescriptionBefore") }}
          <code>0</code>
          {{ t("admin.gatewaySettings.authFailCacheDescriptionAfter") }}
        </template>
      </GatewayNumberSettingRow>

      <FeatureSwitchRow
        :title="t('admin.gatewaySettings.throttleTitle')"
        :description="t('admin.gatewaySettings.throttleDescription')"
        :model-value="form.reverse_proxy_throttle.enabled"
        :disabled="isGatewaySettingsBusy"
        @change="form.reverse_proxy_throttle.enabled = $event"
      />

      <div
        v-show="form.reverse_proxy_throttle.enabled"
        class="divide-y animate-in fade-in slide-in-from-top-2 duration-300"
      >
        <GatewayNumberSettingRow
          v-model="form.reverse_proxy_throttle.requests_per_second"
          :title="t('admin.gatewaySettings.requestsPerSecond')"
          unit-label="req/s"
          :disabled="isGatewaySettingsBusy"
        >
          <template #description>
            {{ t("admin.gatewaySettings.requestsPerSecondDescription") }}
          </template>
        </GatewayNumberSettingRow>

        <GatewayNumberSettingRow
          v-model="form.reverse_proxy_throttle.burst"
          :title="t('admin.gatewaySettings.burst')"
          unit-label="tokens"
          :disabled="isGatewaySettingsBusy"
        >
          <template #description>
            {{ t("admin.gatewaySettings.burstDescription") }}
          </template>
        </GatewayNumberSettingRow>

        <GatewayNumberSettingRow
          v-model="form.reverse_proxy_throttle.block_seconds"
          :title="t('admin.gatewaySettings.blockSeconds')"
          :unit-label="t('admin.gatewaySettings.seconds')"
          unit-width-class="w-12"
          :disabled="isGatewaySettingsBusy"
        >
          <template #description>
            {{ t("admin.gatewaySettings.blockSecondsDescription") }}
          </template>
        </GatewayNumberSettingRow>
      </div>

      <FeatureSwitchRow
        :title="t('admin.gatewaySettings.crawlerBlockerTitle')"
        :description="t('admin.gatewaySettings.crawlerBlockerDescription')"
        :model-value="form.crawler_blocker.enabled"
        :disabled="isGatewaySettingsBusy"
        @change="form.crawler_blocker.enabled = $event"
      />

      <GatewayPortalSummaryRow
        :enabled="portalSummary?.enabled !== false"
        :show-app-icon="portalSummary?.show_app_icon !== false"
        :enabled-label="portalEnabledSummary"
        :display-label="portalDisplaySummary"
        :version-label="portalVersionSummary"
        :icon-label="portalIconSummary"
        @action="openPortalEditor"
      />

      <GatewayUnmatchedRouteSettingRow
        v-model="form.unmatched_route.behavior"
        :title="t('admin.gatewaySettings.unmatchedRoute')"
        :description="t('admin.gatewaySettings.unmatchedRouteDescription')"
        :error-page-label="t('admin.gatewaySettings.unmatchedRouteErrorPage')"
        :reset-connection-label="t('admin.gatewaySettings.unmatchedRouteReset')"
        :warning="t('admin.gatewaySettings.unmatchedRouteDefaultDomainWarning')"
        :disabled="isGatewaySettingsBusy"
      />

      <GatewayUpstreamErrorSettingRow
        v-model="form.unmatched_route.upstream_error_detail"
        :disabled="isGatewaySettingsBusy"
      />

      <GatewayEditorRow
        :title="t('admin.gatewaySettings.visibility')"
        :description="t('admin.gatewaySettings.visibilityDescription')"
        :action-label="t('admin.gatewaySettings.editVisibility')"
        @action="openVisibilityEditor"
      >
        <template #badges>
          <Badge
            :variant="visibilitySummary?.enabled ? 'default' : 'secondary'"
            class="rounded-full px-2.5"
          >
            {{
              visibilitySummary?.enabled
                ? t("admin.gatewaySettings.enabled")
                : t("admin.gatewaySettings.disabled")
            }}
          </Badge>
        </template>
      </GatewayEditorRow>

      <GatewayProxyProtocolSummaryRow
        :summary="proxyProtocolSummary"
        @action="openProxyProtocolEditor"
      />

      <GatewayEditorRow
        :title="t('admin.gatewaySettings.proxyHeaders')"
        :description="t('admin.gatewaySettings.proxyHeadersDescription')"
        :action-label="t('admin.gatewaySettings.editProxyHeaders')"
        :disabled="!isProxyHeadersAvailable"
        :disabled-reason="proxyHeadersDisabledReason"
        @action="openProxyHeadersEditor"
      />

      <GatewayEditorRow
        :title="t('admin.gatewaySettings.hostResponse')"
        :description="t('admin.gatewaySettings.hostResponseDescription')"
        :action-label="t('admin.gatewaySettings.editHostResponse')"
        :disabled="!isHostResponseAvailable"
        :disabled-reason="hostResponseDisabledReason"
        @action="openHostResponseEditor"
      />

      <FloatingActionDock
        :active="isDirty"
        inline-class="flex items-center justify-end gap-3 p-6"
      >
        <template #inline>
          <Button
            variant="outline"
            :disabled="!isDirty || isGatewaySettingsBusy"
            @click="resetForm"
          >
            {{ t("admin.gatewaySettings.reset") }}
          </Button>
          <Button
            :disabled="!isDirty || isGatewaySettingsBusy"
            @click="saveSettings"
          >
            {{ t("admin.gatewaySettings.saveSettings") }}
          </Button>
        </template>
      </FloatingActionDock>
    </CardContent>
  </Card>
</template>
