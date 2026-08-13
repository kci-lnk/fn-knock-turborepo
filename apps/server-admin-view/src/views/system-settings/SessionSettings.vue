<script setup lang="ts">
import { computed, onMounted, reactive, ref, useId } from "vue";
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
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "@/lib/api/config";
import type { AuthCredentialSettings, PostLoginIpGrantMode } from "../../types";
import { useConfigStore } from "../../store/config";
import SessionDurationFieldRow from "./SessionDurationFieldRow.vue";
import {
  durationUnits,
  ipGrantDurationUnits,
  mobilityWindowDurationUnits,
  splitDuration,
  toDurationSeconds as toSeconds,
  type SessionDurationField as DurationField,
} from "./session-settings/sessionDurationModel";
import { useSessionCookieScope } from "./session-settings/useSessionCookieScope";

const a11yId = useId();

const configStore = useConfigStore();
const { t } = useI18n();
const settings = ref<AuthCredentialSettings | null>(null);

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
    description: t(
      "admin.sessionSettings.grantModes.followSession.description",
    ),
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

const formatDuration = (seconds: number, units = durationUnits): string => {
  const normalized = splitDuration(seconds, units);
  const label =
    units.find((item) => item.value === normalized.unit)?.labelKey || "";
  const unitLabel = label ? t(label) : normalized.unit;
  return `${normalized.value} ${unitLabel}`;
};

const {
  effectiveSharedCookieDomain,
  incompatibleCookieScopeHosts,
  isDirectMode,
  isSubdomainRoutingMode,
} = useSessionCookieScope();
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
        duration: formatDuration(
          customGrantTtlSeconds.value,
          ipGrantDurationUnits,
        ),
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
      <div class="border-b border-border bg-muted/20 px-6 py-5">
        <Alert
          class="items-start rounded-xl border-border/70 bg-muted/30 text-foreground shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <AlertTitle>
            {{ t("admin.sessionSettings.newSessionsOnlyTitle") }}
          </AlertTitle>
          <AlertDescription class="text-sm leading-6">
            {{ t("admin.sessionSettings.newSessionsOnlyDescription") }}
          </AlertDescription>
        </Alert>
      </div>

      <SessionDurationFieldRow
        v-model="form.session"
        :title="t('admin.sessionSettings.sessionTtl')"
        :description="t('admin.sessionSettings.sessionTtlDescription')"
        :units="durationUnits"
        :disabled="isSaving"
        :summary="
          t('admin.sessionSettings.willSaveAs', {
            duration: formatDuration(sessionTtlSeconds),
          })
        "
      />

      <SessionDurationFieldRow
        v-model="form.rememberMe"
        :title="t('admin.sessionSettings.rememberMeTtl')"
        :description="t('admin.sessionSettings.rememberMeTtlDescription')"
        :units="durationUnits"
        :disabled="isSaving"
        :summary="
          t('admin.sessionSettings.willSaveAs', {
            duration: formatDuration(rememberMeTtlSeconds),
          })
        "
      />

      <div class="space-y-4 p-6">
        <div
          v-if="isDirectMode"
          class="rounded-xl border border-border bg-muted/20 px-4 py-4"
        >
          <div class="text-sm font-medium text-foreground">
            {{ t("admin.sessionSettings.directModeTitle") }}
          </div>
          <div class="mt-1 text-sm leading-6 text-muted-foreground">
            {{ t("admin.sessionSettings.directModeDescription") }}
          </div>
        </div>

        <div class="space-y-1">
          <div class="text-base font-medium">
            {{ t("admin.sessionSettings.postLoginIpGrantMode") }}
          </div>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.sessionSettings.postLoginIpGrantModeDescription") }}
          </div>
        </div>

        <div
          role="group"
          :aria-label="t('admin.sessionSettings.postLoginIpGrantMode')"
          class="grid gap-3 md:grid-cols-3"
        >
          <button
            v-for="option in postLoginIpGrantModeOptions"
            :key="option.value"
            type="button"
            class="rounded-xl border px-4 py-4 text-left transition-colors"
            :class="
              form.postLoginIpGrantMode === option.value
                ? 'border-primary bg-primary/5'
                : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
            "
            :disabled="isSaving"
            :aria-pressed="form.postLoginIpGrantMode === option.value"
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

        <SessionDurationFieldRow
          v-if="form.postLoginIpGrantMode === 'custom'"
          v-model="form.customGrant"
          :title="t('admin.sessionSettings.customGrantDuration')"
          :description="
            t('admin.sessionSettings.customGrantDurationDescription')
          "
          :units="ipGrantDurationUnits"
          :disabled="isSaving"
          framed
        />

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
              <Label :for="`${a11yId}-sessionsettings-1`" class="text-base">
                {{ t("admin.sessionSettings.sessionIpMobility") }}
              </Label>
              <div class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.sessionSettings.sessionIpMobilityDescription") }}
              </div>
            </div>

            <Switch
              :id="`${a11yId}-sessionsettings-1`"
              class="shrink-0 sm:justify-self-end"
              :model-value="form.sessionIpMobilityEnabled"
              :disabled="isSaving"
              @update:model-value="
                form.sessionIpMobilityEnabled = $event === true
              "
            />
          </div>

          <SessionDurationFieldRow
            v-if="form.sessionIpMobilityEnabled"
            v-model="form.sessionIpMobilityWindow"
            class="mt-4"
            :title="t('admin.sessionSettings.ipRetentionTime')"
            :description="t('admin.sessionSettings.ipRetentionTimeDescription')"
            :units="mobilityWindowDurationUnits"
            :disabled="isSaving"
            framed
          />

          <div class="mt-3 text-sm leading-6 text-muted-foreground">
            {{ sessionIpMobilitySummary }}
          </div>
        </div>

        <div
          v-if="
            form.postLoginIpGrantMode === 'disabled' && isSubdomainRoutingMode
          "
          class="rounded-lg border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground"
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

    <FloatingActionDock
      :active="isDirty"
      inline-class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6"
    >
      <template #inline>
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
            variant="outline"
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
      </template>

      <template #floating>
        <Button
          variant="outline"
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
      </template>
    </FloatingActionDock>
  </Card>
</template>
