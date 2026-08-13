<script setup lang="ts">
import { reactive, ref } from "vue";
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
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import RefreshButton from "@/components/RefreshButton.vue";
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
import SmartConnectFormPanel from "./smart-connect/SmartConnectFormPanel.vue";
import { useSmartConnectViewModel } from "./smart-connect/useSmartConnectViewModel";

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
  options: { preserveDraft?: boolean } = {},
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
    fetcher: async (signal) => SystemAPI.getSmartConnectDetails(signal),
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
  if (isStartingInstall.value) return;
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
  void router.push({ path: "/system", query: { tab: "features" } });
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
          <BreadcrumbLink href="#/system">
            {{ t("admin.smartConnectSettings.systemSettings") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=features">
            {{ t("admin.smartConnectSettings.features") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>
            {{ t("admin.smartConnectSettings.title") }}
          </BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader class="space-y-4">
        <div
          class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1.5">
            <CardTitle class="text-xl tracking-tight">
              {{ t("admin.smartConnectSettings.title") }}
            </CardTitle>
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
        <SmartConnectFormPanel
          v-else-if="details"
          v-model="form"
          :can-use-smart-connect="configStore.canUseSmartConnect"
          :capability-blocked-reason="capabilityBlockedReason"
          :details="details"
          :dnsmasq-action-label="dnsmasqActionLabel"
          :dnsmasq-progress="dnsmasqProgress"
          :dnsmasq-status-label="dnsmasqStatusLabel"
          :dnsmasq-status-variant="dnsmasqStatusVariant"
          :dnsmasq-summary-text="dnsmasqSummaryText"
          :is-dirty="isDirty"
          :is-saving="isSaving"
          :is-smart-connect-available="isSmartConnectAvailable"
          :is-starting-install="isStartingInstall"
          :resolved-ip-options="resolvedIpOptions"
          :save-blocked-reason="saveBlockedReason"
          :show-advanced-cards="showAdvancedCards"
          :show-dnsmasq-action="showDnsmasqAction"
          :show-dnsmasq-card="showDnsmasqCard"
          :show-dnsmasq-setup-card="showDnsmasqSetupCard"
          @cancel="cancelAndBack"
          @save="saveSettings"
          @start-install="startDnsmasqInstall"
        />
      </CardContent>
    </Card>
  </div>
</template>
