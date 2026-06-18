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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type { GatewaySettings } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";

const configStore = useConfigStore();
const router = useRouter();
const { t } = useI18n();
type GatewaySettingsForm = Pick<
  GatewaySettings,
  | "auth_cache_ttl_seconds"
  | "auth_cache_unauthorized_ttl_seconds"
  | "portal"
  | "reverse_proxy_throttle"
>;
const settings = ref<GatewaySettings | null>(null);
const form = reactive<GatewaySettingsForm>({
  auth_cache_ttl_seconds: 1,
  auth_cache_unauthorized_ttl_seconds: 1,
  portal: {
    enabled: true,
    display_style: "domain",
    show_app_icon: false,
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
      Number(form.reverse_proxy_throttle.block_seconds)
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

const runTypeLabelKeyMap = {
  0: "admin.gatewaySettings.runTypes.direct",
  1: "admin.gatewaySettings.runTypes.reverse",
  3: "admin.gatewaySettings.runTypes.subdomain",
} as const;

const currentRunTypeLabel = computed(() => {
  const runType = configStore.config?.run_type;
  if (runType === 0 || runType === 1 || runType === 3) {
    return t(runTypeLabelKeyMap[runType]);
  }
  return t("admin.gatewaySettings.runTypes.current");
});

const visibilitySummary = computed(() => settings.value?.visibility ?? null);
const portalSummary = computed(() => settings.value?.portal ?? null);
const portalEnabledSummary = computed(() =>
  portalSummary.value?.enabled !== false
    ? t("admin.gatewaySettings.enabled")
    : t("admin.gatewaySettings.disabled"),
);
const portalDisplaySummary = computed(() =>
  (portalSummary.value?.display_style ?? "domain") === "title"
    ? t("admin.gatewaySettings.portalDisplayTitle")
    : t("admin.gatewaySettings.portalDisplayDomain"),
);
const portalIconSummary = computed(() =>
  portalSummary.value?.show_app_icon
    ? t("admin.gatewaySettings.enabled")
    : t("admin.gatewaySettings.disabled"),
);

const isProxyHeadersAvailable = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const proxyHeadersDisabledReason = computed(() => {
  if (isProxyHeadersAvailable.value) return "";
  return t("admin.gatewaySettings.subdomainOnlyReason", {
    mode: currentRunTypeLabel.value,
  });
});
const isHostResponseAvailable = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const hostResponseDisabledReason = computed(() => {
  if (isHostResponseAvailable.value) return "";
  return t("admin.gatewaySettings.subdomainOnlyReason", {
    mode: currentRunTypeLabel.value,
  });
});
const isLocationsAvailable = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const locationsDisabledReason = computed(() => {
  if (isLocationsAvailable.value) return "";
  return t("admin.gatewaySettings.subdomainOnlyReason", {
    mode: currentRunTypeLabel.value,
  });
});

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

const toggleThrottleEnabled = () => {
  if (isGatewaySettingsBusy.value) return;
  form.reverse_proxy_throttle.enabled = !form.reverse_proxy_throttle.enabled;
};

const buildSettingsSnapshot = (data: GatewaySettings): GatewaySettings => ({
  ...data,
  portal: {
    enabled: data.portal?.enabled !== false,
    display_style: data.portal?.display_style ?? "domain",
    show_app_icon: data.portal?.show_app_icon === true,
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
      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">{{
            t("admin.gatewaySettings.authCacheTitle")
          }}</Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewaySettings.authCacheDescriptionBefore") }}
            <code>0</code>
            {{ t("admin.gatewaySettings.authCacheDescriptionAfter") }}
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.auth_cache_ttl_seconds"
            type="number"
            min="0"
            step="1"
            class="w-24 text-center"
            :disabled="isGatewaySettingsBusy"
          />
          <span class="w-12 text-sm text-muted-foreground">{{
            t("admin.gatewaySettings.seconds")
          }}</span>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          {{ authCacheHint }}
        </div>
      </div>

      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">{{
            t("admin.gatewaySettings.authFailCacheTitle")
          }}</Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewaySettings.authFailCacheDescriptionBefore") }}
            <code>0</code>
            {{ t("admin.gatewaySettings.authFailCacheDescriptionAfter") }}
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.auth_cache_unauthorized_ttl_seconds"
            type="number"
            min="0"
            step="1"
            class="w-24 text-center"
            :disabled="isGatewaySettingsBusy"
          />
          <span class="w-12 text-sm text-muted-foreground">{{
            t("admin.gatewaySettings.seconds")
          }}</span>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          {{ authCacheFailHint }}
        </div>
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="cursor-pointer text-base font-medium"
            @click="toggleThrottleEnabled"
          >
            {{ t("admin.gatewaySettings.throttleTitle") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewaySettings.throttleDescription") }}
          </div>
        </div>
        <Switch
          v-model="form.reverse_proxy_throttle.enabled"
          :disabled="isGatewaySettingsBusy"
        />
      </div>

      <div
        v-show="form.reverse_proxy_throttle.enabled"
        class="divide-y animate-in fade-in slide-in-from-top-2 duration-300"
      >
        <div
          class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">{{
              t("admin.gatewaySettings.requestsPerSecond")
            }}</Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.gatewaySettings.requestsPerSecondDescription") }}
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.reverse_proxy_throttle.requests_per_second"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isGatewaySettingsBusy"
            />
            <span class="w-16 text-sm text-muted-foreground">req/s</span>
          </div>
        </div>

        <div
          class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">{{
              t("admin.gatewaySettings.burst")
            }}</Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.gatewaySettings.burstDescription") }}
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.reverse_proxy_throttle.burst"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isGatewaySettingsBusy"
            />
            <span class="w-16 text-sm text-muted-foreground">tokens</span>
          </div>
        </div>

        <div
          class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">{{
              t("admin.gatewaySettings.blockSeconds")
            }}</Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.gatewaySettings.blockSecondsDescription") }}
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.reverse_proxy_throttle.block_seconds"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isGatewaySettingsBusy"
            />
            <span class="w-12 text-sm text-muted-foreground">{{
              t("admin.gatewaySettings.seconds")
            }}</span>
          </div>
        </div>
      </div>

      <div
        class="grid gap-4 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center"
      >
        <div class="space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <Label class="text-base">{{
              t("admin.gatewaySettings.portal")
            }}</Label>
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
              :variant="portalSummary?.show_app_icon ? 'default' : 'secondary'"
              class="rounded-full px-2.5"
            >
              {{
                t("admin.gatewaySettings.portalIconSummary", {
                  state: portalIconSummary,
                })
              }}
            </Badge>
          </div>
          <div class="text-sm leading-6 text-muted-foreground">
            {{ t("admin.gatewaySettings.portalDescription") }}
          </div>
        </div>
        <div class="flex justify-start lg:justify-end">
          <Button variant="outline" @click="openPortalEditor">{{
            t("admin.gatewaySettings.editPortal")
          }}</Button>
        </div>
      </div>

      <div
        class="grid gap-4 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center"
      >
        <div class="space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <Label class="text-base">{{
              t("admin.gatewaySettings.visibility")
            }}</Label>
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
          </div>
          <div class="text-sm leading-6 text-muted-foreground">
            {{ t("admin.gatewaySettings.visibilityDescription") }}
          </div>
        </div>
        <div class="flex justify-start lg:justify-end">
          <Button variant="outline" @click="openVisibilityEditor">{{
            t("admin.gatewaySettings.editVisibility")
          }}</Button>
        </div>
      </div>

      <div
        class="grid gap-4 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center"
      >
        <div class="space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <Label
              class="text-base"
              :class="isProxyHeadersAvailable ? '' : 'text-zinc-500'"
            >
              {{ t("admin.gatewaySettings.proxyHeaders") }}
            </Label>
          </div>
          <div
            class="text-sm leading-6"
            :class="
              isProxyHeadersAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.gatewaySettings.proxyHeadersDescription") }}
          </div>
          <div
            v-if="!isProxyHeadersAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ proxyHeadersDisabledReason }}
          </div>
        </div>
        <div class="flex justify-start lg:justify-end">
          <Button
            variant="outline"
            :disabled="!isProxyHeadersAvailable"
            @click="openProxyHeadersEditor"
          >
            {{ t("admin.gatewaySettings.editProxyHeaders") }}
          </Button>
        </div>
      </div>

      <div
        class="grid gap-4 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center"
      >
        <div class="space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <Label
              class="text-base"
              :class="isHostResponseAvailable ? '' : 'text-zinc-500'"
            >
              {{ t("admin.gatewaySettings.hostResponse") }}
            </Label>
          </div>
          <div
            class="text-sm leading-6"
            :class="
              isHostResponseAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.gatewaySettings.hostResponseDescription") }}
          </div>
          <div
            v-if="!isHostResponseAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ hostResponseDisabledReason }}
          </div>
        </div>
        <div class="flex justify-start lg:justify-end">
          <Button
            variant="outline"
            :disabled="!isHostResponseAvailable"
            @click="openHostResponseEditor"
          >
            {{ t("admin.gatewaySettings.editHostResponse") }}
          </Button>
        </div>
      </div>

      <div
        class="grid gap-4 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center"
      >
        <div class="space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <Label
              class="text-base"
              :class="isLocationsAvailable ? '' : 'text-zinc-500'"
            >
              {{ t("admin.gatewaySettings.locations") }}
            </Label>
          </div>
          <div
            class="text-sm leading-6"
            :class="
              isLocationsAvailable ? 'text-muted-foreground' : 'text-zinc-500'
            "
          >
            {{ t("admin.gatewaySettings.locationsDescription") }}
          </div>
          <div
            v-if="!isLocationsAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ locationsDisabledReason }}
          </div>
        </div>
        <div class="flex justify-start lg:justify-end">
          <Button
            variant="outline"
            :disabled="!isLocationsAvailable"
            @click="openLocationsEditor"
          >
            {{ t("admin.gatewaySettings.editLocations") }}
          </Button>
        </div>
      </div>

      <div class="flex items-center justify-end gap-3 p-6">
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
      </div>
    </CardContent>
  </Card>
</template>
