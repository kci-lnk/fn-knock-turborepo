<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type { GatewaySettings } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";
import GatewayEditorRow from "./GatewayEditorRow.vue";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";
import GatewayNumberSettingRow from "./GatewayNumberSettingRow.vue";
import { useGatewaySubdomainEditorAvailability } from "./useGatewaySubdomainEditorAvailability";

const configStore = useConfigStore();
const router = useRouter();
const { t } = useI18n();
type GatewaySettingsForm = Pick<
  GatewaySettings,
  | "auth_cache_ttl_seconds"
  | "auth_cache_unauthorized_ttl_seconds"
  | "portal"
  | "crawler_blocker"
  | "reverse_proxy_throttle"
>;
const settings = ref<GatewaySettings | null>(null);
const form = reactive<GatewaySettingsForm>({
  auth_cache_ttl_seconds: 1,
  auth_cache_unauthorized_ttl_seconds: 1,
  portal: {
    enabled: true,
    display_style: "title",
    show_app_icon: true,
    icon_drag_mode: "corners",
  },
  crawler_blocker: {
    enabled: false,
    updated_at: null,
  },
  reverse_proxy_throttle: {
    enabled: true,
    requests_per_second: 100,
    burst: 200,
    block_seconds: 30,
  },
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewaySettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewaySettings.loadFailedDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewaySettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewaySettings.saveSettingsFailedDescription"),
      ),
    });
  },
});
const isGatewaySettingsBusy = computed(() => isSaving.value);

const clampCacheTtl = (value: unknown) =>
  Math.max(0, Math.floor(Number(value) || 0));

const clampPositiveInt = (value: unknown, fallback = 1) =>
  Math.max(1, Math.floor(Number(value) || fallback));

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.auth_cache_ttl_seconds !==
      Number(form.auth_cache_ttl_seconds) ||
    settings.value.auth_cache_unauthorized_ttl_seconds !==
      Number(form.auth_cache_unauthorized_ttl_seconds) ||
    settings.value.reverse_proxy_throttle.enabled !==
      form.reverse_proxy_throttle.enabled ||
    settings.value.reverse_proxy_throttle.requests_per_second !==
      Number(form.reverse_proxy_throttle.requests_per_second) ||
    settings.value.reverse_proxy_throttle.burst !==
      Number(form.reverse_proxy_throttle.burst) ||
    settings.value.reverse_proxy_throttle.block_seconds !==
      Number(form.reverse_proxy_throttle.block_seconds) ||
    settings.value.crawler_blocker.enabled !== form.crawler_blocker.enabled
  );
});

const authCacheHint = computed(() =>
  Number(form.auth_cache_ttl_seconds) === 0
    ? t("admin.gatewaySettings.authCacheDisabled")
    : t("admin.gatewaySettings.authCacheEnabled", {
        seconds: clampCacheTtl(form.auth_cache_ttl_seconds),
      }),
);

const authCacheFailHint = computed(() =>
  Number(form.auth_cache_unauthorized_ttl_seconds) === 0
    ? t("admin.gatewaySettings.authCacheFailDisabled")
    : t("admin.gatewaySettings.authCacheFailEnabled", {
        seconds: clampCacheTtl(form.auth_cache_unauthorized_ttl_seconds),
      }),
);

const visibilitySummary = computed(() => settings.value?.visibility ?? null);
const portalSummary = computed(() => settings.value?.portal ?? null);
const portalEnabledSummary = computed(() =>
  portalSummary.value?.enabled !== false
    ? t("admin.gatewaySettings.enabled")
    : t("admin.gatewaySettings.disabled"),
);
const portalDisplaySummary = computed(() =>
  portalSummary.value?.display_style === "domain"
    ? t("admin.gatewaySettings.portalDisplayDomain")
    : t("admin.gatewaySettings.portalDisplayTitle"),
);
const portalIconSummary = computed(() =>
  portalSummary.value?.show_app_icon !== false
    ? t("admin.gatewaySettings.enabled")
    : t("admin.gatewaySettings.disabled"),
);

const {
  isProxyHeadersAvailable,
  proxyHeadersDisabledReason,
  isHostResponseAvailable,
  hostResponseDisabledReason,
  isLocationsAvailable,
  locationsDisabledReason,
} = useGatewaySubdomainEditorAvailability();

const openVisibilityEditor = () => {
  void router.push("/system/gateway-visibility");
};

const openPortalEditor = () => {
  void router.push("/system/gateway-portal");
};

const openProxyHeadersEditor = () => {
  if (!isProxyHeadersAvailable.value) {
    return;
  }

  void router.push("/system/gateway-proxy-headers");
};

const openHostResponseEditor = () => {
  if (!isHostResponseAvailable.value) {
    return;
  }

  void router.push("/system/gateway-host-response");
};

const openLocationsEditor = () => {
  if (!isLocationsAvailable.value) {
    return;
  }

  void router.push("/system/gateway-locations");
};

const buildSettingsSnapshot = (data: GatewaySettings): GatewaySettings => ({
  ...data,
  portal: {
    enabled: data.portal?.enabled !== false,
    display_style: data.portal?.display_style === "domain" ? "domain" : "title",
    show_app_icon: data.portal?.show_app_icon !== false,
    icon_drag_mode: data.portal?.icon_drag_mode === "free" ? "free" : "corners",
  },
  crawler_blocker: {
    enabled: data.crawler_blocker?.enabled === true,
    updated_at: data.crawler_blocker?.updated_at ?? null,
  },
  reverse_proxy_throttle: { ...data.reverse_proxy_throttle },
});

const applySettingsSnapshot = (data: GatewaySettings) => {
  const snapshot = buildSettingsSnapshot(data);
  settings.value = snapshot;
  return snapshot;
};

const applyFromSettings = (data: GatewaySettings) => {
  const snapshot = applySettingsSnapshot(data);
  form.auth_cache_ttl_seconds = data.auth_cache_ttl_seconds;
  form.auth_cache_unauthorized_ttl_seconds =
    data.auth_cache_unauthorized_ttl_seconds;
  form.portal.enabled = snapshot.portal.enabled;
  form.portal.display_style = snapshot.portal.display_style;
  form.portal.show_app_icon = snapshot.portal.show_app_icon;
  form.portal.icon_drag_mode = snapshot.portal.icon_drag_mode;
  form.crawler_blocker.enabled = snapshot.crawler_blocker.enabled;
  form.crawler_blocker.updated_at = snapshot.crawler_blocker.updated_at;
  form.reverse_proxy_throttle.enabled = data.reverse_proxy_throttle.enabled;
  form.reverse_proxy_throttle.requests_per_second =
    data.reverse_proxy_throttle.requests_per_second;
  form.reverse_proxy_throttle.burst = data.reverse_proxy_throttle.burst;
  form.reverse_proxy_throttle.block_seconds =
    data.reverse_proxy_throttle.block_seconds;
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await ConfigAPI.getGatewaySettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  await runSaveSettings(
    () =>
      ConfigAPI.updateGatewaySettings({
        auth_cache_ttl_seconds: clampCacheTtl(form.auth_cache_ttl_seconds),
        auth_cache_unauthorized_ttl_seconds: clampCacheTtl(
          form.auth_cache_unauthorized_ttl_seconds,
        ),
        reverse_proxy_throttle: {
          enabled: form.reverse_proxy_throttle.enabled,
          requests_per_second: clampPositiveInt(
            form.reverse_proxy_throttle.requests_per_second,
            100,
          ),
          burst: clampPositiveInt(form.reverse_proxy_throttle.burst, 200),
          block_seconds: clampPositiveInt(
            form.reverse_proxy_throttle.block_seconds,
            30,
          ),
        },
        crawler_blocker: {
          enabled: form.crawler_blocker.enabled,
        },
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await configStore.loadConfig();
        toast.success(t("admin.gatewaySettings.settingsUpdated"));
      },
    },
  );
};

onMounted(fetchSettings);
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

      <GatewayEditorRow
        :title="t('admin.gatewaySettings.portal')"
        :description="t('admin.gatewaySettings.portalDescription')"
        :action-label="t('admin.gatewaySettings.editPortal')"
        @action="openPortalEditor"
      >
        <template #badges>
          <Badge
            :variant="
              portalSummary?.enabled !== false ? 'default' : 'secondary'
            "
            class="rounded-full px-2.5"
          >
            {{ portalEnabledSummary }}
          </Badge>
          <Badge variant="secondary" class="rounded-full px-2.5">
            {{ portalDisplaySummary }}
          </Badge>
          <Badge
            :variant="
              portalSummary?.show_app_icon !== false ? 'default' : 'secondary'
            "
            class="rounded-full px-2.5"
          >
            {{
              t("admin.gatewaySettings.portalIconSummary", {
                state: portalIconSummary,
              })
            }}
          </Badge>
        </template>
      </GatewayEditorRow>

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

      <GatewayEditorRow
        :title="t('admin.gatewaySettings.locations')"
        :description="t('admin.gatewaySettings.locationsDescription')"
        :action-label="t('admin.gatewaySettings.editLocations')"
        :disabled="!isLocationsAvailable"
        :disabled-reason="locationsDisabledReason"
        @action="openLocationsEditor"
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
