<script setup lang="ts">
import { reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import RefreshButton from "@/components/RefreshButton.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { toast } from "@admin-shared/utils/toast";
import { SystemAPI } from "@/lib/api/system";
import type { SmartConnectConfig, SmartConnectDetails } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { usePollingResourceStatus } from "@admin-shared/composables/usePollingResourceStatus";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";
import {
  cloneSmartConnectDetails,
  hasUnsavedSmartConnectDraft,
  resolveSelectedIpv4,
} from "./smart-connect/smartConnectModel";
import { useSmartConnectViewModel } from "./smart-connect/useSmartConnectViewModel";

const a11yId = useId();

const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();
const details = ref<SmartConnectDetails | null>(null);
const loadError = ref("");
const form = reactive<SmartConnectConfig>({
  enabled: false,
  selected_ipv4: "",
});

const applyDetails = (
  value: SmartConnectDetails,
  options: {
    preserveDraft?: boolean;
  } = {},
) => {
  const nextDetails = cloneSmartConnectDetails(value);
  const shouldPreserveDraft =
    options.preserveDraft === true &&
    hasUnsavedSmartConnectDraft(details.value, form);
  const selectedIpv4 = shouldPreserveDraft
    ? form.selected_ipv4
    : nextDetails.config.selected_ipv4;

  details.value = nextDetails;
  form.enabled = shouldPreserveDraft
    ? form.enabled
    : nextDetails.config.enabled;
  form.selected_ipv4 = resolveSelectedIpv4(
    selectedIpv4,
    nextDetails.local_ip_options,
  );
};

const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.smartConnectSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.smartConnectSettings.saveFailedDescription"),
      ),
    });
    void refreshDetails();
  },
});

const { isPending: isStartingInstall, run: runStartInstall } = useAsyncAction({
  onError: (error) => {
    const installMode = details.value?.dnsmasq.installed
      ? "initialize"
      : "install";
    toast.error(
      installMode === "initialize"
        ? t("admin.smartConnectSettings.initializeFailed")
        : t("admin.smartConnectSettings.installFailed"),
      {
        description: extractErrorMessage(
          error,
          installMode === "initialize"
            ? t("admin.smartConnectSettings.startInitializeFailed")
            : t("admin.smartConnectSettings.startInstallFailed"),
        ),
      },
    );
    void refreshDetails();
  },
});

const { isInitializing, refresh: refreshDetails } =
  usePollingResourceStatus<SmartConnectDetails>({
    fetcher: async (signal) => {
      const data = await SystemAPI.getSmartConnectDetails(signal);
      return data;
    },
    onData: (value) => {
      loadError.value = "";
      applyDetails(value, { preserveDraft: true });
    },
    isDownloading: (value) =>
      value.dnsmasq.install_state.status === "installing",
    onError: (error) => {
      loadError.value = extractErrorMessage(
        error,
        t("admin.smartConnectSettings.loadFailedDescription"),
      );
    },
  });

const showLoadingSkeleton = useDelayedLoading(isInitializing);

const {
  capabilityBlockedReason,
  dnsmasqActionLabel,
  dnsmasqProgress,
  dnsmasqStatusLabel,
  dnsmasqStatusVariant,
  dnsmasqSummaryText,
  isDirty,
  isSmartConnectAvailable,
  resolvedIpOptions,
  saveBlockedReason,
  showAdvancedCards,
  showDnsmasqAction,
  showDnsmasqCard,
  showDnsmasqSetupCard,
} = useSmartConnectViewModel({ details, form });

const refreshAll = async () => {
  await Promise.all([refreshDetails(), configStore.loadConfig()]);
};

const startDnsmasqInstall = async () => {
  if (isStartingInstall.value) {
    return;
  }

  const installMode = details.value?.dnsmasq.installed
    ? "initialize"
    : "install";
  await runStartInstall(() => SystemAPI.installDnsmasq(), {
    onSuccess: async (state) => {
      toast.success(
        state.status === "installed"
          ? installMode === "initialize"
            ? t("admin.smartConnectSettings.dnsmasqInitialized")
            : t("admin.smartConnectSettings.dnsmasqReady")
          : installMode === "initialize"
            ? t("admin.smartConnectSettings.dnsmasqInitializeStarted")
            : t("admin.smartConnectSettings.dnsmasqInstallStarted"),
      );
      await refreshDetails();
    },
  });
};

const cancelAndBack = () => {
  void router.push({
    path: "/system",
    query: {
      tab: "features",
    },
  });
};

const saveSettings = async () => {
  if (saveBlockedReason.value) {
    toast.error(t("admin.smartConnectSettings.cannotSaveNow"), {
      description: saveBlockedReason.value,
    });
    return;
  }

  await runSave(
    () =>
      SystemAPI.updateSmartConnect({
        enabled: form.enabled,
        selected_ipv4: form.enabled ? form.selected_ipv4 : undefined,
      }),
    {
      onSuccess: async (value) => {
        applyDetails(value);
        await configStore.loadConfig();
        toast.success(t("admin.smartConnectSettings.updatedAndSynced"));
      },
    },
  );
};
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.smartConnectSettings.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=features">{{
            t("admin.smartConnectSettings.features")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.smartConnectSettings.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader class="space-y-4">
        <div
          class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1.5">
            <div class="flex flex-wrap items-center gap-2">
              <CardTitle class="text-xl tracking-tight">{{
                t("admin.smartConnectSettings.title")
              }}</CardTitle>
            </div>
            <CardDescription class="max-w-2xl leading-6">
              {{ t("admin.smartConnectSettings.description") }}
            </CardDescription>
          </div>
          <RefreshButton
            :loading="isInitializing"
            :disabled="isSaving || isStartingInstall"
            @click="refreshAll"
          />
        </div>
      </CardHeader>

      <CardContent class="space-y-5">
        <div
          v-if="!details && isInitializing && showLoadingSkeleton"
          class="space-y-4"
        >
          <Skeleton class="h-28 w-full rounded-2xl" />
          <Skeleton class="h-56 w-full rounded-2xl" />
        </div>

        <div
          v-else-if="loadError && !details"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-5 py-4 text-sm text-destructive"
          role="alert"
        >
          {{ loadError }}
        </div>

        <template v-else-if="details">
          <div
            v-if="!isSmartConnectAvailable || !configStore.canUseSmartConnect"
            class="rounded-xl border border-zinc-200 bg-zinc-50 px-4 py-3 text-sm leading-6 text-zinc-700"
          >
            {{
              !configStore.canUseSmartConnect
                ? capabilityBlockedReason
                : t(
                    "admin.smartConnectSettings.currentModeUnavailableWithReason",
                    {
                      reason: details.availability.reason,
                    },
                  )
            }}
          </div>

          <div
            class="rounded-2xl border border-border/60 bg-muted/10 px-4 py-4"
          >
            <div class="flex items-start justify-between gap-4">
              <div class="min-w-0 space-y-2">
                <div class="flex flex-wrap items-center gap-2">
                  <Label
                    :for="`${a11yId}-smartconnectsettings-1`"
                    class="text-base font-medium"
                    >{{ t("admin.smartConnectSettings.title") }}</Label
                  >
                </div>
              </div>

              <Switch
                :id="`${a11yId}-smartconnectsettings-1`"
                class="mt-0.5 shrink-0"
                :model-value="form.enabled"
                :disabled="
                  !configStore.canUseSmartConnect ||
                  isSaving ||
                  isStartingInstall
                "
                @update:model-value="form.enabled = $event === true"
              />
            </div>
          </div>

          <div class="overflow-hidden rounded-xl border border-border/60">
            <template v-if="showDnsmasqCard">
              <section v-if="showDnsmasqSetupCard" class="space-y-4 p-5">
                <div
                  class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
                >
                  <div class="space-y-1">
                    <div class="flex flex-wrap items-center gap-2">
                      <div class="text-base font-medium">
                        {{ t("admin.smartConnectSettings.runtimeEnvironment") }}
                      </div>
                      <Badge :variant="dnsmasqStatusVariant">
                        {{ dnsmasqStatusLabel }}
                      </Badge>
                    </div>
                    <p class="text-sm leading-6 text-muted-foreground">
                      {{ details.dnsmasq.install_state.message }}
                    </p>
                    <p class="text-xs leading-5 text-muted-foreground">
                      {{ dnsmasqSummaryText }}
                    </p>
                  </div>

                  <Button
                    v-if="showDnsmasqAction"
                    class="w-full sm:w-auto"
                    :disabled="
                      isSaving ||
                      isStartingInstall ||
                      details.dnsmasq.install_state.status === 'installing'
                    "
                    @click="startDnsmasqInstall"
                  >
                    <span
                      v-if="
                        isStartingInstall ||
                        details.dnsmasq.install_state.status === 'installing'
                      "
                      class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                    ></span>
                    {{ dnsmasqActionLabel }}
                  </Button>
                </div>

                <div
                  v-if="details.dnsmasq.install_state.status === 'installing'"
                >
                  <Progress :model-value="dnsmasqProgress" />
                </div>
              </section>

              <template v-if="showAdvancedCards">
                <section
                  :class="[
                    'space-y-4 p-5',
                    showDnsmasqSetupCard ? 'border-t border-border/60' : '',
                  ]"
                >
                  <div
                    class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(280px,360px)] lg:items-start lg:gap-6"
                  >
                    <div class="space-y-1">
                      <Label
                        :for="`${a11yId}-smartconnectsettings-2`"
                        class="text-base"
                        >{{ t("admin.smartConnectSettings.localLanIp") }}</Label
                      >
                      <p class="text-sm leading-6 text-muted-foreground">
                        {{ t("admin.smartConnectSettings.localLanIpHint") }}
                      </p>
                    </div>

                    <div class="space-y-2">
                      <Select
                        :model-value="form.selected_ipv4 || undefined"
                        @update:model-value="
                          form.selected_ipv4 = String($event ?? '')
                        "
                      >
                        <SelectTrigger
                          :id="`${a11yId}-smartconnectsettings-2`"
                          class="h-10 w-full rounded-lg border-border/70 bg-background px-3 text-sm shadow-none"
                        >
                          <SelectValue
                            :placeholder="
                              t('admin.smartConnectSettings.selectLocalIpv4')
                            "
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem
                            v-for="option in resolvedIpOptions"
                            :key="`${option.interface}-${option.value}`"
                            :value="option.value"
                          >
                            {{ option.label }}
                          </SelectItem>
                        </SelectContent>
                      </Select>

                      <p
                        v-if="resolvedIpOptions.length === 0"
                        class="text-sm leading-6 text-muted-foreground"
                      >
                        {{ t("admin.smartConnectSettings.noPrivateIpv4") }}
                      </p>
                    </div>
                  </div>
                </section>

                <section class="space-y-4 border-t border-border/60 p-5">
                  <div class="space-y-1">
                    <div class="text-base font-medium">
                      {{ t("admin.smartConnectSettings.syncedDomains") }}
                    </div>
                    <p class="text-sm leading-6 text-muted-foreground">
                      {{ t("admin.smartConnectSettings.syncedDomainsHint") }}
                    </p>
                  </div>

                  <div class="rounded-xl bg-muted/20 px-4 py-4">
                    <div
                      v-if="details.domains.length === 0"
                      class="text-sm leading-6 text-muted-foreground"
                    >
                      {{ t("admin.smartConnectSettings.noSyncedDomains") }}
                    </div>
                    <div v-else class="flex flex-wrap gap-2">
                      <Badge
                        v-for="domain in details.domains"
                        :key="domain"
                        variant="secondary"
                        class="max-w-full break-all"
                      >
                        {{ domain }}
                      </Badge>
                    </div>
                  </div>
                </section>

                <section class="space-y-4 border-t border-border/60 p-5">
                  <div class="space-y-1">
                    <div class="text-base font-medium">
                      {{ t("admin.smartConnectSettings.notes") }}
                    </div>
                    <p class="text-sm leading-6 text-muted-foreground">
                      {{
                        t("admin.smartConnectSettings.dnsInstruction", {
                          ip:
                            form.selected_ipv4 ||
                            t("admin.smartConnectSettings.localLanIpFallback"),
                        })
                      }}

                      <span>{{
                        t("admin.smartConnectSettings.androidWarning")
                      }}</span>
                    </p>
                  </div>
                </section>
              </template>
            </template>

            <FloatingActionDock
              :active="isDirty"
              :inline-class="[
                'space-y-4 p-5',
                showDnsmasqCard ? 'border-t border-border/60' : '',
              ]"
            >
              <template #inline>
                <div
                  class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
                >
                  <p class="text-sm leading-6 text-muted-foreground">
                    {{
                      saveBlockedReason ||
                      t("admin.smartConnectSettings.saveSyncHint")
                    }}
                  </p>

                  <div class="flex gap-3 sm:ml-auto">
                    <Button
                      variant="outline"
                      :disabled="isSaving"
                      @click="cancelAndBack"
                    >
                      {{ t("common.cancel") }}
                    </Button>
                    <Button
                      :disabled="
                        !isDirty ||
                        isSaving ||
                        isStartingInstall ||
                        Boolean(saveBlockedReason)
                      "
                      @click="saveSettings"
                    >
                      <span
                        v-if="isSaving"
                        class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                      ></span>
                      {{
                        isSaving
                          ? t("admin.smartConnectSettings.saving")
                          : t("admin.smartConnectSettings.saveAndSync")
                      }}
                    </Button>
                  </div>
                </div>
              </template>

              <template #floating>
                <Button
                  variant="outline"
                  :disabled="isSaving"
                  @click="cancelAndBack"
                >
                  {{ t("common.cancel") }}
                </Button>
                <Button
                  :disabled="
                    !isDirty ||
                    isSaving ||
                    isStartingInstall ||
                    Boolean(saveBlockedReason)
                  "
                  @click="saveSettings"
                >
                  <span
                    v-if="isSaving"
                    class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                  ></span>
                  {{
                    isSaving
                      ? t("admin.smartConnectSettings.saving")
                      : t("admin.smartConnectSettings.saveAndSync")
                  }}
                </Button>
              </template>
            </FloatingActionDock>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
