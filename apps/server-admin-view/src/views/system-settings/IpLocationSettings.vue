<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  CheckCircle2,
  ExternalLink,
  Link2,
  LoaderCircle,
  RotateCcw,
  Save,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupInput } from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { IpLocationSettingsAPI } from "../../lib/api";
import type { IpLocationApiConfig } from "../../lib/api";

const OFFICIAL_IP_LOOKUP_URL = "https://ipaddress.fnknock.cn/api/v1";
const OFFICIAL_CIDR_URL = "https://cidr.fnknock.cn/api/v1";
const DEFAULT_CUSTOM_IP_LOOKUP_URL = "http://127.0.0.1:30661";
const DEFAULT_CUSTOM_CIDR_URL = "http://127.0.0.1:30662";
const ipLookupDockerUrl = "https://hub.docker.com/r/kcilnk/go-ipaddress-api";
const cidrDockerUrl = "https://hub.docker.com/r/kcilnk/go-cidr-api";
const { t } = useI18n();

const settings = ref<IpLocationApiConfig | null>(null);
const form = reactive<
  Pick<IpLocationApiConfig, "ip_lookup_mode" | "cidr_mode">
>({
  ip_lookup_mode: "online",
  cidr_mode: "online",
});

const ipLookupUrlInput = ref("");
const cidrUrlInput = ref("");

const normalizeBaseUrl = (value: string) => value.trim().replace(/\/+$/, "");

const applyDefaultCustomUrls = () => {
  if (
    form.ip_lookup_mode === "custom" &&
    !normalizeBaseUrl(ipLookupUrlInput.value)
  ) {
    ipLookupUrlInput.value = DEFAULT_CUSTOM_IP_LOOKUP_URL;
  }

  if (form.cidr_mode === "custom" && !normalizeBaseUrl(cidrUrlInput.value)) {
    cidrUrlInput.value = DEFAULT_CUSTOM_CIDR_URL;
  }
};

const buildPayload = (): IpLocationApiConfig => ({
  ip_lookup_mode: form.ip_lookup_mode,
  ip_lookup_url:
    form.ip_lookup_mode === "custom"
      ? normalizeBaseUrl(ipLookupUrlInput.value)
      : OFFICIAL_IP_LOOKUP_URL,
  cidr_mode: form.cidr_mode,
  cidr_url:
    form.cidr_mode === "custom"
      ? normalizeBaseUrl(cidrUrlInput.value)
      : OFFICIAL_CIDR_URL,
});

const currentPayload = computed(buildPayload);

const isHttpUrl = (value: string) => {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
};

const validateCustomUrls = (payload: IpLocationApiConfig) => {
  if (payload.ip_lookup_mode === "custom") {
    if (!payload.ip_lookup_url) {
      toast.error(t("admin.ipLocationSettings.ipLookupUrlRequired"));
      return false;
    }
    if (!isHttpUrl(payload.ip_lookup_url)) {
      toast.error(t("admin.ipLocationSettings.ipLookupUrlInvalid"), {
        description: t("admin.ipLocationSettings.httpUrlRequired"),
      });
      return false;
    }
  }

  if (payload.cidr_mode === "custom") {
    if (!payload.cidr_url) {
      toast.error(t("admin.ipLocationSettings.cidrUrlRequired"));
      return false;
    }
    if (!isHttpUrl(payload.cidr_url)) {
      toast.error(t("admin.ipLocationSettings.cidrUrlInvalid"), {
        description: t("admin.ipLocationSettings.httpUrlRequired"),
      });
      return false;
    }
  }

  return true;
};

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipLocationSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ipLocationSettings.loadFailedDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);

const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipLocationSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ipLocationSettings.saveFailedDescription"),
      ),
    });
  },
});

const { isPending: isTestingIpLookup, run: runTestIpLookup } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipLocationSettings.connectionFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ipLocationSettings.ipLookupUnavailable"),
      ),
    });
  },
});

const { isPending: isTestingCidr, run: runTestCidr } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ipLocationSettings.connectionFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ipLocationSettings.cidrUnavailable"),
      ),
    });
  },
});

const isDirty = computed(() => {
  if (!settings.value) return false;
  const payload = currentPayload.value;
  return (
    settings.value.ip_lookup_mode !== payload.ip_lookup_mode ||
    settings.value.ip_lookup_url !== payload.ip_lookup_url ||
    settings.value.cidr_mode !== payload.cidr_mode ||
    settings.value.cidr_url !== payload.cidr_url
  );
});

const applyFromSettings = (data: IpLocationApiConfig) => {
  const normalized: IpLocationApiConfig = {
    ip_lookup_mode: data.ip_lookup_mode,
    ip_lookup_url: normalizeBaseUrl(data.ip_lookup_url),
    cidr_mode: data.cidr_mode,
    cidr_url: normalizeBaseUrl(data.cidr_url),
  };

  settings.value = normalized;
  form.ip_lookup_mode = normalized.ip_lookup_mode;
  form.cidr_mode = normalized.cidr_mode;
  ipLookupUrlInput.value =
    normalized.ip_lookup_mode === "custom" ? normalized.ip_lookup_url : "";
  cidrUrlInput.value =
    normalized.cidr_mode === "custom" ? normalized.cidr_url : "";
  applyDefaultCustomUrls();
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await IpLocationSettingsAPI.getSettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const testIpLookupService = async () => {
  const url = normalizeBaseUrl(ipLookupUrlInput.value);
  if (!url) {
    toast.error(t("admin.ipLocationSettings.ipLookupUrlInputRequired"));
    return;
  }
  if (!isHttpUrl(url)) {
    toast.error(t("admin.ipLocationSettings.ipLookupUrlInvalid"), {
      description: t("admin.ipLocationSettings.httpUrlRequired"),
    });
    return;
  }

  await runTestIpLookup(async () => {
    const result = await IpLocationSettingsAPI.testIpLookup(url);
    if (result.success) {
      toast.success(t("admin.ipLocationSettings.connectionSuccess"), {
        description: t("admin.ipLocationSettings.ipLookupHealthy"),
      });
    } else {
      toast.error(t("admin.ipLocationSettings.connectionFailed"), {
        description: result.message,
      });
    }
  });
};

const testCidrService = async () => {
  const url = normalizeBaseUrl(cidrUrlInput.value);
  if (!url) {
    toast.error(t("admin.ipLocationSettings.cidrUrlInputRequired"));
    return;
  }
  if (!isHttpUrl(url)) {
    toast.error(t("admin.ipLocationSettings.cidrUrlInvalid"), {
      description: t("admin.ipLocationSettings.httpUrlRequired"),
    });
    return;
  }

  await runTestCidr(async () => {
    const result = await IpLocationSettingsAPI.testCidr(url);
    if (result.success) {
      toast.success(t("admin.ipLocationSettings.connectionSuccess"), {
        description: t("admin.ipLocationSettings.cidrHealthy"),
      });
    } else {
      toast.error(t("admin.ipLocationSettings.connectionFailed"), {
        description: result.message,
      });
    }
  });
};

const saveSettings = async () => {
  const payload = currentPayload.value;
  if (!validateCustomUrls(payload)) return;

  await runSaveSettings(() => IpLocationSettingsAPI.updateSettings(payload), {
    onSuccess: (data) => {
      applyFromSettings(data);
      toast.success(t("admin.ipLocationSettings.settingsUpdated"));
    },
  });
};

watch(
  () => [form.ip_lookup_mode, form.cidr_mode] as const,
  applyDefaultCustomUrls,
);

onMounted(fetchSettings);
</script>

<template>
  <div class="w-full space-y-4">
    <div
      v-if="isLoading && showLoadingSkeleton"
      class="grid gap-4"
    >
      <section class="rounded-xl border bg-card p-5 shadow-sm">
        <div class="flex gap-3">
          <Skeleton class="size-10 rounded-lg" />
          <div class="flex-1 space-y-2">
            <Skeleton class="h-5 w-32" />
            <Skeleton class="h-4 w-4/5" />
          </div>
        </div>
        <div class="mt-6 space-y-3">
          <Skeleton class="h-4 w-24" />
          <Skeleton class="h-9 w-full" />
          <Skeleton class="h-20 w-full" />
        </div>
      </section>
      <section class="rounded-xl border bg-card p-5 shadow-sm">
        <div class="flex gap-3">
          <Skeleton class="size-10 rounded-lg" />
          <div class="flex-1 space-y-2">
            <Skeleton class="h-5 w-32" />
            <Skeleton class="h-4 w-4/5" />
          </div>
        </div>
        <div class="mt-6 space-y-3">
          <Skeleton class="h-4 w-24" />
          <Skeleton class="h-9 w-full" />
          <Skeleton class="h-20 w-full" />
        </div>
      </section>
    </div>

    <div v-else-if="!isLoading" class="grid gap-4">
      <section class="flex min-h-full flex-col overflow-hidden rounded-xl border bg-card shadow-sm">
        <div class="border-b bg-muted/10 p-4 sm:p-5">
          <div class="flex gap-3">
            <div class="min-w-0 space-y-1">
              <h3 class="text-base font-semibold tracking-normal">
                {{ t("admin.ipLocationSettings.ipLookupTitle") }}
              </h3>
              <p class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.ipLocationSettings.ipLookupDescription") }}
              </p>
            </div>
          </div>
        </div>

        <div class="flex w-full flex-1 flex-col gap-5 p-4 sm:p-5">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3">
              <Label for="ip-location-lookup-mode" class="text-sm font-medium">
                {{ t("admin.ipLocationSettings.serviceSource") }}
              </Label>
            </div>
            <Select v-model="form.ip_lookup_mode" :disabled="isSaving">
              <SelectTrigger id="ip-location-lookup-mode" class="w-full">
                <SelectValue
                  :placeholder="t('admin.ipLocationSettings.chooseServiceSource')"
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="online">{{
                  t("admin.ipLocationSettings.officialOnlineService")
                }}</SelectItem>
                <SelectItem value="custom">{{
                  t("admin.ipLocationSettings.customService")
                }}</SelectItem>
              </SelectContent>
            </Select>
            <p class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.ipLocationSettings.ipLookupModeHint") }}
            </p>
          </div>

          <div
            v-if="form.ip_lookup_mode === 'online'"
            class="mt-auto rounded-lg border border-dashed bg-muted/20 p-4"
          >
            <div class="flex gap-3">
              <CheckCircle2 class="mt-0.5 size-4 shrink-0 text-emerald-600" />
              <div class="min-w-0 space-y-1">
                <p class="text-sm font-medium">
                  {{ t("admin.ipLocationSettings.usingOfficialService") }}
                </p>
              </div>
            </div>
          </div>

          <div
            v-if="form.ip_lookup_mode === 'custom'"
            class="animate-in fade-in slide-in-from-top-2 space-y-4 duration-200"
          >
            <div class="rounded-lg border bg-muted/20 p-4">
              <div class="flex gap-3">
                <div class="space-y-1 text-sm">
                  <p class="font-medium">
                    {{ t("admin.ipLocationSettings.selfHostedService") }}
                  </p>
                  <p class="leading-6 text-muted-foreground">
                    {{ t("admin.ipLocationSettings.canUse") }}
                    <a
                      :href="ipLookupDockerUrl"
                      target="_blank"
                      rel="noopener noreferrer"
                      class="inline-flex items-center gap-1 text-primary hover:underline"
                    >
                      go-ipaddress-api
                      <ExternalLink class="size-3.5" />
                    </a>
                    {{ t("admin.ipLocationSettings.deploySuffix") }}
                  </p>
                </div>
              </div>
            </div>

            <div class="space-y-2">
              <Label for="ip-location-lookup-url" class="text-sm font-medium">
                Base URL
              </Label>
              <div class="flex flex-col gap-2 sm:flex-row">
                <InputGroup class="sm:flex-1">
                  <InputGroupInput
                    id="ip-location-lookup-url"
                    v-model="ipLookupUrlInput"
                    :placeholder="
                      t('admin.ipLocationSettings.ipLookupPlaceholder')
                    "
                    :disabled="isSaving"
                  />
                </InputGroup>
                <Button
                  variant="outline"
                  class="w-full sm:w-auto"
                  :disabled="isTestingIpLookup || !ipLookupUrlInput.trim()"
                  @click="testIpLookupService"
                >
                  <LoaderCircle
                    v-if="isTestingIpLookup"
                    class="size-4 animate-spin"
                  />
                  <Link2 v-else class="size-4" />
                  {{
                    isTestingIpLookup
                      ? t("admin.ipLocationSettings.testing")
                      : t("admin.ipLocationSettings.testConnection")
                  }}
                </Button>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.ipLocationSettings.baseUrlHint") }}
              </p>
            </div>
          </div>
        </div>
      </section>

      <section class="flex min-h-full flex-col overflow-hidden rounded-xl border bg-card shadow-sm">
        <div class="border-b bg-muted/10 p-4 sm:p-5">
          <div class="flex gap-3">
            <div class="min-w-0 space-y-1">
              <h3 class="text-base font-semibold tracking-normal">
                {{ t("admin.ipLocationSettings.cidrTitle") }}
              </h3>
              <p class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.ipLocationSettings.cidrDescription") }}
              </p>
            </div>
          </div>
        </div>

        <div class="flex w-full flex-1 flex-col gap-5 p-4 sm:p-5">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3">
              <Label for="ip-location-cidr-mode" class="text-sm font-medium">
                {{ t("admin.ipLocationSettings.serviceSource") }}
              </Label>
            </div>
            <Select v-model="form.cidr_mode" :disabled="isSaving">
              <SelectTrigger id="ip-location-cidr-mode" class="w-full">
                <SelectValue
                  :placeholder="t('admin.ipLocationSettings.chooseServiceSource')"
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="online">{{
                  t("admin.ipLocationSettings.officialOnlineService")
                }}</SelectItem>
                <SelectItem value="custom">{{
                  t("admin.ipLocationSettings.customService")
                }}</SelectItem>
              </SelectContent>
            </Select>
            <p class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.ipLocationSettings.cidrModeHint") }}
            </p>
          </div>

          <div
            v-if="form.cidr_mode === 'online'"
            class="mt-auto rounded-lg border border-dashed bg-muted/20 p-4"
          >
            <div class="flex gap-3">
              <CheckCircle2 class="mt-0.5 size-4 shrink-0 text-emerald-600" />
              <div class="min-w-0 space-y-1">
                <p class="text-sm font-medium">
                  {{ t("admin.ipLocationSettings.usingOfficialService") }}
                </p>
              </div>
            </div>
          </div>

          <div
            v-if="form.cidr_mode === 'custom'"
            class="animate-in fade-in slide-in-from-top-2 space-y-4 duration-200"
          >
            <div class="rounded-lg border bg-muted/20 p-4">
              <div class="flex gap-3">
                <div class="space-y-1 text-sm">
                  <p class="font-medium">
                    {{ t("admin.ipLocationSettings.selfHostedService") }}
                  </p>
                  <p class="leading-6 text-muted-foreground">
                    {{ t("admin.ipLocationSettings.canUse") }}
                  <a
                    :href="cidrDockerUrl"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="inline-flex items-center gap-1 text-primary hover:underline"
                  >
                    go-cidr-api
                    <ExternalLink class="size-3.5" />
                  </a>
                  {{ t("admin.ipLocationSettings.deploySuffix") }}
                  </p>
                </div>
              </div>
            </div>

            <div class="space-y-2">
              <Label for="ip-location-cidr-url" class="text-sm font-medium">
                Base URL
              </Label>
              <div class="flex flex-col gap-2 sm:flex-row">
                <InputGroup class="sm:flex-1">
                  <InputGroupInput
                    id="ip-location-cidr-url"
                    v-model="cidrUrlInput"
                    :placeholder="t('admin.ipLocationSettings.cidrPlaceholder')"
                    :disabled="isSaving"
                  />
                </InputGroup>
                <Button
                  variant="outline"
                  class="w-full sm:w-auto"
                  :disabled="isTestingCidr || !cidrUrlInput.trim()"
                  @click="testCidrService"
                >
                  <LoaderCircle
                    v-if="isTestingCidr"
                    class="size-4 animate-spin"
                  />
                  <Link2 v-else class="size-4" />
                  {{
                    isTestingCidr
                      ? t("admin.ipLocationSettings.testing")
                      : t("admin.ipLocationSettings.testConnection")
                  }}
                </Button>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.ipLocationSettings.baseUrlHint") }}
              </p>
            </div>
          </div>
        </div>
      </section>
    </div>

    <div v-else class="min-h-[220px] rounded-xl border bg-card" aria-hidden="true" />

    <FloatingActionDock
      :active="isDirty"
      inline-class="rounded-xl border bg-card px-4 py-4 shadow-sm sm:px-6"
    >
      <template #inline>
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-end">
          <div class="flex gap-3">
            <Button
              variant="outline"
              class="flex-1 sm:flex-none"
              @click="resetForm"
              :disabled="!isDirty || isSaving"
            >
              <RotateCcw class="size-4" />
              {{ t("admin.ipLocationSettings.discard") }}
            </Button>
            <Button
              class="flex-1 sm:flex-none"
              :disabled="!isDirty || isSaving"
              @click="saveSettings"
            >
              <LoaderCircle v-if="isSaving" class="size-4 animate-spin" />
              <Save v-else class="size-4" />
              {{
                isSaving
                  ? t("admin.ipLocationSettings.saving")
                  : t("admin.ipLocationSettings.saveChanges")
              }}
            </Button>
          </div>
        </div>
      </template>

      <template #floating>
        <Button
          variant="outline"
          @click="resetForm"
          :disabled="!isDirty || isSaving"
        >
          <RotateCcw class="size-4" />
          {{ t("admin.ipLocationSettings.discard") }}
        </Button>
        <Button
          :disabled="!isDirty || isSaving"
          @click="saveSettings"
        >
          <LoaderCircle v-if="isSaving" class="size-4 animate-spin" />
          <Save v-else class="size-4" />
          {{
            isSaving
              ? t("admin.ipLocationSettings.saving")
              : t("admin.ipLocationSettings.saveChanges")
          }}
        </Button>
      </template>
    </FloatingActionDock>
  </div>
</template>
