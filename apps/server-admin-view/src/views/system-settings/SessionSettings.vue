<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Info } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../../lib/api";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import type {
  AuthCredentialSettings,
  HostMapping,
  PostLoginIpGrantMode,
} from "../../types";
import { useConfigStore } from "../../store/config";

type DurationUnit = "second" | "minute" | "hour" | "day" | "week" | "year";

type DurationField = {
  value: number;
  unit: DurationUnit;
};

const configStore = useConfigStore();
const { t } = useI18n();
const settings = ref<AuthCredentialSettings | null>(null);

const durationUnits: Array<{
  value: DurationUnit;
  labelKey: string;
  seconds: number;
}> = [
  { value: "second", labelKey: "admin.sessionSettings.units.second", seconds: 1 },
  { value: "minute", labelKey: "admin.sessionSettings.units.minute", seconds: 60 },
  { value: "hour", labelKey: "admin.sessionSettings.units.hour", seconds: 3600 },
  { value: "day", labelKey: "admin.sessionSettings.units.day", seconds: 24 * 3600 },
  { value: "week", labelKey: "admin.sessionSettings.units.week", seconds: 7 * 24 * 3600 },
  { value: "year", labelKey: "admin.sessionSettings.units.year", seconds: 365 * 24 * 3600 },
];

const ipGrantDurationUnits = durationUnits.filter(
  (unit) =>
    unit.value === "second" || unit.value === "minute" || unit.value === "hour",
);
const mobilityWindowDurationUnits = durationUnits.filter(
  (unit) => unit.value === "minute" || unit.value === "hour",
);

const durationUnitMap = Object.fromEntries(
  durationUnits.map((item) => [item.value, item.seconds]),
) as Record<DurationUnit, number>;

const form = reactive<{
  session: DurationField;
  rememberMe: DurationField;
  postLoginIpGrantMode: PostLoginIpGrantMode;
  customGrant: DurationField;
  sessionIpMobilityEnabled: boolean;
  sessionIpMobilityWindow: DurationField;
}>({
  session: {
    value: 24,
    unit: "hour",
  },
  rememberMe: {
    value: 1,
    unit: "year",
  },
  postLoginIpGrantMode: "follow_session",
  customGrant: {
    value: 1,
    unit: "hour",
  },
  sessionIpMobilityEnabled: false,
  sessionIpMobilityWindow: {
    value: 20,
    unit: "minute",
  },
});

const postLoginIpGrantModeOptions = computed<
  Array<{
    value: PostLoginIpGrantMode;
    title: string;
    description: string;
  }>
>(() => [
  {
    value: "follow_session",
    title: t("admin.sessionSettings.grantModes.followSession.title"),
    description: t("admin.sessionSettings.grantModes.followSession.description"),
  },
  {
    value: "disabled",
    title: t("admin.sessionSettings.grantModes.disabled.title"),
    description: t("admin.sessionSettings.grantModes.disabled.description"),
  },
  {
    value: "custom",
    title: t("admin.sessionSettings.grantModes.custom.title"),
    description: t("admin.sessionSettings.grantModes.custom.description"),
  },
]);

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessionSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessionSettings.loadDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessionSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessionSettings.saveDescription"),
      ),
    });
  },
});

const clampDurationValue = (value: unknown) =>
  Math.max(1, Math.floor(Number(value) || 0));

const toSeconds = (field: DurationField): number =>
  clampDurationValue(field.value) * durationUnitMap[field.unit];

const splitDuration = (
  seconds: number,
  units = durationUnits,
): DurationField => {
  const safeSeconds = Math.max(1, Math.floor(Number(seconds) || 1));
  const matchedUnit =
    [...units].reverse().find((unit) => safeSeconds % unit.seconds === 0) ??
    units[0] ??
    durationUnits[0]!;

  return {
    value: Math.max(1, safeSeconds / matchedUnit.seconds),
    unit: matchedUnit.value,
  };
};

const formatDuration = (seconds: number, units = durationUnits): string => {
  const normalized = splitDuration(seconds, units);
  const label =
    units.find((item) => item.value === normalized.unit)?.labelKey ||
    "";
  const unitLabel = label ? t(label) : normalized.unit;
  return `${normalized.value} ${unitLabel}`;
};

const isDirectMode = computed(() => configStore.config?.run_type === 0);
const isSubdomainRoutingMode = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const normalizeDomainName = (value: string | null | undefined) =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^\./, "")
    .replace(/\.$/, "");
const isHostWithinDomain = (host: string, domain: string): boolean => {
  const normalizedHost = normalizeDomainName(host);
  const normalizedDomain = normalizeDomainName(domain);
  if (!normalizedHost || !normalizedDomain) return false;
  return (
    normalizedHost === normalizedDomain ||
    normalizedHost.endsWith(`.${normalizedDomain}`)
  );
};
const isAuthServiceMapping = (mapping: HostMapping): boolean =>
  mapping.service_role === "auth";
const effectiveSharedCookieDomain = computed(() => {
  const explicit = configStore.config?.subdomain_mode?.cookie_domain?.trim();
  if (explicit) return explicit;
  const rootDomain = configStore.config?.subdomain_mode?.root_domain?.trim();
  return rootDomain || "";
});
const incompatibleCookieScopeHosts = computed(() => {
  if (!isSubdomainRoutingMode.value) return [];
  const sharedDomain = normalizeDomainName(effectiveSharedCookieDomain.value);
  const mappings = configStore.config?.host_mappings ?? [];
  return mappings
    .filter((mapping) => mapping.use_auth && !isAuthServiceMapping(mapping))
    .map((mapping) => normalizeDomainName(mapping.host))
    .filter(
      (host): host is string =>
        Boolean(host) &&
        (!sharedDomain || !isHostWithinDomain(host, sharedDomain)),
    );
});
const sessionTtlSeconds = computed(() => toSeconds(form.session));
const rememberMeTtlSeconds = computed(() => toSeconds(form.rememberMe));
const customGrantTtlSeconds = computed(() => toSeconds(form.customGrant));
const sessionIpMobilityWindowSeconds = computed(() =>
  toSeconds(form.sessionIpMobilityWindow),
);

const isDirty = computed(() => {
  if (!settings.value) return false;
  const storedGrantTtl = settings.value.post_login_ip_grant_ttl_seconds ?? 3600;
  const shouldCompareCustomGrantTtl =
    settings.value.post_login_ip_grant_mode === "custom" ||
    form.postLoginIpGrantMode === "custom";
  return (
    settings.value.session_ttl_seconds !== sessionTtlSeconds.value ||
    settings.value.remember_me_ttl_seconds !== rememberMeTtlSeconds.value ||
    settings.value.post_login_ip_grant_mode !== form.postLoginIpGrantMode ||
    settings.value.session_ip_mobility_enabled !==
      form.sessionIpMobilityEnabled ||
    settings.value.session_ip_mobility_window_seconds !==
      sessionIpMobilityWindowSeconds.value ||
    (shouldCompareCustomGrantTtl &&
      storedGrantTtl !== customGrantTtlSeconds.value)
  );
});

const grantModeSummary = computed(() => {
  switch (form.postLoginIpGrantMode) {
    case "follow_session":
      return t("admin.sessionSettings.grantSummary.followSession");
    case "disabled":
      return t("admin.sessionSettings.grantSummary.disabled");
    case "custom":
      return t("admin.sessionSettings.grantSummary.custom", {
        duration: formatDuration(customGrantTtlSeconds.value, ipGrantDurationUnits),
      });
    default:
      return "";
  }
});

const sessionIpMobilitySummary = computed(() => {
  if (!form.sessionIpMobilityEnabled) {
    return t("admin.sessionSettings.mobilitySummary.disabled");
  }

  return t("admin.sessionSettings.mobilitySummary.enabled", {
    duration: formatDuration(
      sessionIpMobilityWindowSeconds.value,
      mobilityWindowDurationUnits,
    ),
  });
});

const applyFromSettings = (data: AuthCredentialSettings) => {
  settings.value = data;
  Object.assign(form.session, splitDuration(data.session_ttl_seconds));
  Object.assign(form.rememberMe, splitDuration(data.remember_me_ttl_seconds));
  form.postLoginIpGrantMode = data.post_login_ip_grant_mode;
  Object.assign(
    form.customGrant,
    splitDuration(
      data.post_login_ip_grant_ttl_seconds ?? 3600,
      ipGrantDurationUnits,
    ),
  );
  form.sessionIpMobilityEnabled = data.session_ip_mobility_enabled === true;
  Object.assign(
    form.sessionIpMobilityWindow,
    splitDuration(
      data.session_ip_mobility_window_seconds ?? 20 * 60,
      mobilityWindowDurationUnits,
    ),
  );
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await ConfigAPI.getAuthCredentialSettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  const nextSessionTtl = sessionTtlSeconds.value;
  const nextRememberMeTtl = rememberMeTtlSeconds.value;
  const nextCustomGrantTtl = customGrantTtlSeconds.value;
  const nextMobilityWindowSeconds = sessionIpMobilityWindowSeconds.value;

  if (nextSessionTtl < 60 || nextRememberMeTtl < 60) {
    toast.error(t("admin.sessionSettings.tooShort"), {
      description: t("admin.sessionSettings.sessionTooShortDescription"),
    });
    return;
  }

  if (nextRememberMeTtl < nextSessionTtl) {
    toast.error(t("admin.sessionSettings.invalidSettings"), {
      description: t("admin.sessionSettings.rememberMeShorterDescription"),
    });
    return;
  }

  if (form.postLoginIpGrantMode === "custom" && nextCustomGrantTtl < 60) {
    toast.error(t("admin.sessionSettings.invalidSettings"), {
      description: t("admin.sessionSettings.customGrantTooShortDescription"),
    });
    return;
  }

  if (
    form.sessionIpMobilityEnabled &&
    (nextMobilityWindowSeconds < 60 || nextMobilityWindowSeconds > 24 * 3600)
  ) {
    toast.error(t("admin.sessionSettings.invalidSettings"), {
      description: t("admin.sessionSettings.mobilityWindowInvalidDescription"),
    });
    return;
  }

  await runSaveSettings(
    () =>
      ConfigAPI.updateAuthCredentialSettings({
        session_ttl_seconds: nextSessionTtl,
        remember_me_ttl_seconds: nextRememberMeTtl,
        post_login_ip_grant_mode: form.postLoginIpGrantMode,
        post_login_ip_grant_ttl_seconds:
          form.postLoginIpGrantMode === "custom" ? nextCustomGrantTtl : null,
        session_ip_mobility_enabled: form.sessionIpMobilityEnabled,
        session_ip_mobility_window_seconds: nextMobilityWindowSeconds,
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await configStore.loadConfig();
        toast.success(t("admin.sessionSettings.updated"));
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">
        {{ t("admin.sessionSettings.title") }}
      </CardTitle>
      <CardDescription class="mt-1.5">
        {{ t("admin.sessionSettings.description") }}
      </CardDescription>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div class="border-b border-zinc-200 bg-zinc-50/40 px-6 py-5">
        <Alert
          class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <AlertTitle>
            {{ t("admin.sessionSettings.newSessionsOnlyTitle") }}
          </AlertTitle>
          <AlertDescription class="text-sm leading-6 text-zinc-700">
            {{ t("admin.sessionSettings.newSessionsOnlyDescription") }}
          </AlertDescription>
        </Alert>
      </div>

      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">
            {{ t("admin.sessionSettings.sessionTtl") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessionSettings.sessionTtlDescription") }}
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.session.value"
            type="number"
            min="1"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <Select v-model="form.session.unit" :disabled="isSaving">
            <SelectTrigger class="w-[110px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="unit in durationUnits"
                :key="unit.value"
                :value="unit.value"
              >
                {{ t(unit.labelKey) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          {{
            t("admin.sessionSettings.willSaveAs", {
              duration: formatDuration(sessionTtlSeconds),
            })
          }}
        </div>
      </div>

      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">
            {{ t("admin.sessionSettings.rememberMeTtl") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessionSettings.rememberMeTtlDescription") }}
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.rememberMe.value"
            type="number"
            min="1"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <Select v-model="form.rememberMe.unit" :disabled="isSaving">
            <SelectTrigger class="w-[110px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="unit in durationUnits"
                :key="unit.value"
                :value="unit.value"
              >
                {{ t(unit.labelKey) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          {{
            t("admin.sessionSettings.willSaveAs", {
              duration: formatDuration(rememberMeTtlSeconds),
            })
          }}
        </div>
      </div>

      <div class="space-y-4 p-6">
        <div
          v-if="isDirectMode"
          class="rounded-xl border border-zinc-200 bg-zinc-50/40 px-4 py-4"
        >
          <div class="text-sm font-medium text-zinc-900">
            {{ t("admin.sessionSettings.directModeTitle") }}
          </div>
          <div class="mt-1 text-sm leading-6 text-zinc-700">
            {{ t("admin.sessionSettings.directModeDescription") }}
          </div>
        </div>

        <div class="space-y-1">
          <Label class="text-base">
            {{ t("admin.sessionSettings.postLoginIpGrantMode") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessionSettings.postLoginIpGrantModeDescription") }}
          </div>
        </div>

        <div class="grid gap-3 md:grid-cols-3">
          <button
            v-for="option in postLoginIpGrantModeOptions"
            :key="option.value"
            type="button"
            class="rounded-xl border px-4 py-4 text-left transition-colors"
            :class="
              form.postLoginIpGrantMode === option.value
                ? 'border-primary bg-primary/5'
                : 'border-border bg-background hover:border-zinc-300'
            "
            :disabled="isSaving"
            @click="form.postLoginIpGrantMode = option.value"
          >
            <div class="text-sm font-medium text-foreground">
              {{ option.title }}
            </div>
            <div class="mt-1 text-sm leading-6 text-muted-foreground">
              {{ option.description }}
            </div>
          </button>
        </div>

        <div
          v-if="form.postLoginIpGrantMode === 'custom'"
          class="grid gap-3 rounded-xl border bg-muted/15 p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">
              {{ t("admin.sessionSettings.customGrantDuration") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.sessionSettings.customGrantDurationDescription") }}
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.customGrant.value"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isSaving"
            />
            <Select v-model="form.customGrant.unit" :disabled="isSaving">
              <SelectTrigger class="w-[110px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="unit in ipGrantDurationUnits"
                  :key="unit.value"
                  :value="unit.value"
                >
                  {{ t(unit.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div
          class="rounded-lg bg-muted/20 px-4 py-3 text-sm text-muted-foreground"
        >
          {{ grantModeSummary }}
        </div>

        <div class="border-t border-border/60 pt-5">
          <div
            class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
          >
            <div class="space-y-1 pr-6">
              <Label class="text-base">
                {{ t("admin.sessionSettings.sessionIpMobility") }}
              </Label>
              <div class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.sessionSettings.sessionIpMobilityDescription") }}
              </div>
            </div>

            <Switch
              class="shrink-0 sm:justify-self-end"
              :model-value="form.sessionIpMobilityEnabled"
              :disabled="isSaving"
              @update:model-value="
                form.sessionIpMobilityEnabled = $event === true
              "
            />
          </div>

          <div
            v-if="form.sessionIpMobilityEnabled"
            class="mt-4 grid gap-3 rounded-xl border bg-muted/15 p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
          >
            <div class="space-y-1 pr-6">
              <Label class="text-base">
                {{ t("admin.sessionSettings.ipRetentionTime") }}
              </Label>
              <div class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.sessionSettings.ipRetentionTimeDescription") }}
              </div>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <Input
                v-model.number="form.sessionIpMobilityWindow.value"
                type="number"
                min="1"
                step="1"
                class="w-24 text-center"
                :disabled="isSaving"
              />
              <Select
                v-model="form.sessionIpMobilityWindow.unit"
                :disabled="isSaving"
              >
                <SelectTrigger class="w-[110px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="unit in mobilityWindowDurationUnits"
                    :key="unit.value"
                    :value="unit.value"
                  >
                    {{ t(unit.labelKey) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div class="mt-3 text-sm leading-6 text-muted-foreground">
            {{ sessionIpMobilitySummary }}
          </div>
        </div>

        <div
          v-if="
            form.postLoginIpGrantMode === 'disabled' && isSubdomainRoutingMode
          "
          class="rounded-lg border border-zinc-200 bg-zinc-50/40 px-4 py-3 text-sm text-zinc-700"
        >
          <template v-if="effectiveSharedCookieDomain">
            {{ t("admin.sessionSettings.sharedCookiePrefix") }}
            <code>{{ effectiveSharedCookieDomain }}</code>
            {{ t("admin.sessionSettings.sharedCookieSuffix") }}
            <template v-if="incompatibleCookieScopeHosts.length > 0">
              {{ t("admin.sessionSettings.incompatibleHostsPrefix") }}
              <code>{{
                incompatibleCookieScopeHosts.join(
                  t("admin.sessionSettings.listSeparator"),
                )
              }}</code
              >{{ t("admin.sessionSettings.incompatibleHostsSuffix") }}
            </template>
            <template v-else>
              {{ t("admin.sessionSettings.allHostsCompatible") }}
            </template>
          </template>
          <template v-else>
            {{ t("admin.sessionSettings.noSharedCookieDomain") }}
          </template>
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <div
      class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6"
    >
      <div class="text-sm text-muted-foreground">
        <span v-if="isDirty">
          {{ t("admin.sessionSettings.unsavedChanges") }}
        </span>
        <span v-else>
          {{ t("admin.sessionSettings.upToDate") }}
        </span>
      </div>
      <div class="flex gap-3">
        <Button
          variant="ghost"
          @click="resetForm"
          :disabled="!isDirty || isSaving"
        >
          {{ t("admin.sessionSettings.discard") }}
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.sessionSettings.saveChanges") }}
        </Button>
      </div>
    </div>
  </Card>
</template>
