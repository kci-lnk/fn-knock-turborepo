<template>
  <Card
    v-if="
      !hasLoadedSSLStatus || (isLoading && showLoadingSkeleton && !sslStatus)
    "
    class="dynamic-white-cert-card"
  >
    <CardHeader>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <CardTitle>{{ t("admin.certConfig.title") }}</CardTitle>
        </div>
      </div>
      <CardDescription>{{ t("common.loadingConfig") }}</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-4">
      <div
        class="dynamic-white-cert-subsurface grid gap-3 rounded-lg border bg-muted/30 p-4"
      >
        <div
          class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
        >
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-56" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-64" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-40" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-48" />
        </div>
      </div>
    </CardContent>
  </Card>

  <div v-else class="grid gap-4">
    <CertificateStatusCard
      :active-certificate="activeCertificate"
      :certificate-count="certificates.length"
      :deployment-mode-label="deploymentModeLabel"
      :is-activating="isActivating"
      :is-clearing="isClearing"
      :is-updating-deployment-mode="isUpdatingDeploymentMode"
      :library-coverage="libraryCoverage"
      :primary-certificate-badge-label="primaryCertificateBadgeLabel"
      :recommended-certificate-id="recommendedCertificateId"
      :show-multi-sni-suggestion="showMultiSniSuggestion"
      :status-overview-text="statusOverviewText"
      @activate-recommended="activateRecommendedCertificate"
      @clear="handleClear"
      @switch-to-multi-sni="updateDeploymentMode('multi_sni')"
    />

    <CertificateDeploymentCard
      :certificate-count="certificates.length"
      :configured-deployment-mode-label="configuredDeploymentModeLabel"
      :deployed-gateway-certificates="deployedGatewayCertificates"
      :deployment-card-class="deploymentCardClass"
      :deployment-mode-description="deploymentModeDescription"
      :deployment-mode-mismatch="deploymentModeMismatch"
      :deployment-mode-short-label="deploymentModeShortLabel"
      :deployment-section-configured="deploymentSectionConfigured"
      :deployment-summary="deploymentSummary"
      :gateway-certificate-key="gatewayCertificateKey"
      :gateway-certificate-label="gatewayCertificateLabel"
      :gateway-deployment-summary="gatewayDeploymentSummary"
      :gateway-sync-error="gatewaySyncError"
      :is-updating-deployment-mode="isUpdatingDeploymentMode"
      :multi-sni-preview="multiSniPreview"
      :pending-deployment-mode="pendingDeploymentMode"
      :ready="hasLoadedSSLStatus"
      :single-active-preview="singleActivePreview"
      :ssl-status="sslStatus"
      @update-mode="updateDeploymentMode"
    />

    <ActiveCertificateDetailsCard
      :active-certificate="activeCertificate"
      :coverage-badge-class="coverageBadgeClass"
      :coverage-badge-label="coverageBadgeLabel"
      :coverage-badge-variant="coverageBadgeVariant"
      :current-certificate-summary="currentCertificateSummary"
      :format-date="formatDate"
      :format-dn="formatDN"
      :is-expired="isExpired"
      :is-expiring-soon="isExpiringSoon"
      :ready="hasLoadedSSLStatus"
      :source-label="sourceLabel"
      :subdomain-coverage="subdomainCoverage"
      :uncovered-hosts-preview="uncoveredHostsPreview"
    />

    <ConfigCollapsibleCard
      :title="t('admin.certConfig.manualUploadTitle')"
      :configured="manualUploadConfigured"
      :ready="hasLoadedSSLStatus"
      :edit-label="t('admin.certConfig.expandForm')"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
      card-class="dynamic-white-cert-card"
    >
      <template #summary>{{ manualUploadSummary }}</template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="grid gap-2 p-4 sm:p-6">
            <div class="text-base font-semibold">
              {{ t("admin.certConfig.uploadNewTitle") }}
            </div>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.certConfig.uploadDescription") }}
            </p>
          </div>

          <div class="grid gap-6 p-4 sm:p-6">
            <CertForm
              v-model:cert="formData.cert"
              v-model:sslKey="formData.key"
              :share-name="sslSharedFiles.shareName"
              :shared-files="sslSharedFiles.files"
              :shared-files-available="sslSharedFiles.available"
              :shared-files-loading="isLoadingSharedFiles"
              :shared-files-error="sharedFilesError"
              :shared-file-selecting="isReadingSharedFile"
              @request-shared-files="handleSharedFilesRequest"
              @select-shared-file="handleCreateSharedFileSelect"
            />

            <Alert v-if="errorMessage" variant="destructive">
              <AlertTitle>{{
                t("admin.certConfig.validationFailed")
              }}</AlertTitle>
              <AlertDescription>{{ errorMessage }}</AlertDescription>
            </Alert>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">
          {{ t("admin.certConfig.collapse") }}
        </Button>
        <Button
          variant="outline"
          :disabled="isSaving || (!formData.cert && !formData.key)"
          @click="resetManualUploadForm"
        >
          {{ t("admin.certConfig.clear") }}
        </Button>
        <Button
          variant="outline"
          :disabled="isSaving || !formData.cert || !formData.key"
          @click="handleSave(false)"
        >
          <span
            v-if="isSaving && pendingSaveMode === 'store'"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.certConfig.storeOnly") }}
        </Button>
        <Button
          :disabled="isSaving || !formData.cert || !formData.key"
          @click="handleSave(true)"
        >
          <span
            v-if="isSaving && pendingSaveMode === 'activate'"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.certConfig.saveAndActivate") }}
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <CertificateLibraryCard
      :activate-button-label="activateButtonLabel"
      :activate-certificate="activateCertificate"
      :activating-certificate-id="activatingCertificateId"
      :certificate-display-label="certificateDisplayLabel"
      :certificate-role-label="certificateRoleLabel"
      :certificates="certificates"
      :clear-library="handleClearLibrary"
      :coverage-badge-class="coverageBadgeClass"
      :coverage-badge-label="coverageBadgeLabel"
      :coverage-badge-variant="coverageBadgeVariant"
      :delete-certificate="deleteCertificate"
      :deleting-certificate-id="deletingCertificateId"
      :format-date="formatDate"
      :is-activating="isActivating"
      :is-clearing-library="isClearingLibrary"
      :is-deleting="isDeleting"
      :ready="hasLoadedSSLStatus"
      :source-label="sourceLabel"
      :summary="certificateLibrarySummary"
    />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import CertForm from "@admin-shared/components/ssl/CertForm.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "@/lib/api/config";
import type { SSLStatus } from "../../types";
import { toast } from "@admin-shared/utils/toast";
import ActiveCertificateDetailsCard from "./ActiveCertificateDetailsCard.vue";
import CertificateDeploymentCard from "./CertificateDeploymentCard.vue";
import CertificateLibraryCard from "./CertificateLibraryCard.vue";
import CertificateStatusCard from "./CertificateStatusCard.vue";
import { useCertConfigViewModel } from "./useCertConfigViewModel";
import { useSSLSharedFiles } from "./useSSLSharedFiles";

const sslStatus = ref<SSLStatus | null>(null);
const { t, locale } = useI18n();
const hasLoadedSSLStatus = ref(false);
const errorMessage = ref("");
const formData = ref({ cert: "", key: "" });
const pendingSaveMode = ref<"store" | "activate" | null>(null);
const activatingCertificateId = ref<string | null>(null);
const deletingCertificateId = ref<string | null>(null);
const pendingDeploymentMode = ref<"single_active" | "multi_sni" | null>(null);

const {
  handleCreateSharedFileSelect,
  handleSharedFilesRequest,
  isLoadingSharedFiles,
  isReadingSharedFile,
  sharedFilesError,
  sslSharedFiles,
} = useSSLSharedFiles({
  formData,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const { isPending: isSaving, run: runSaveSSL } = useAsyncAction({
  onError: (error) => {
    const message = extractErrorMessage(
      error,
      t("admin.certConfig.saveFailed"),
    );
    errorMessage.value = message;
    toast.error(message);
  },
});
const { isPending: isClearing, run: runClearSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.certConfig.disableFailed")),
    );
  },
});
const { isPending: isLoading, run: runLoadSSLStatus } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to load SSL status:", error);
  },
});
const { isPending: isActivating, run: runActivateSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.certConfig.switchCertificateFailed")),
    );
  },
});
const { isPending: isDeleting, run: runDeleteSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.certConfig.deleteFailed")));
  },
});
const { isPending: isClearingLibrary, run: runClearSSLLibrary } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.certConfig.clearLibraryFailed")),
      );
    },
  });
const { isPending: isUpdatingDeploymentMode, run: runUpdateDeploymentMode } =
  useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.certConfig.switchModeFailed")),
      );
    },
  });

const showLoadingSkeleton = useDelayedLoading(isLoading);

const {
  activateButtonLabel,
  activeCertificate,
  certificateDisplayLabel,
  certificateLibrarySummary,
  certificateRoleLabel,
  certificates,
  configuredDeploymentModeLabel,
  coverageBadgeClass,
  coverageBadgeLabel,
  coverageBadgeVariant,
  currentCertificateSummary,
  deployedGatewayCertificates,
  deploymentCardClass,
  deploymentModeDescription,
  deploymentModeLabel,
  deploymentModeMismatch,
  deploymentModeShortLabel,
  deploymentSectionConfigured,
  deploymentSummary,
  formatDate,
  formatDN,
  gatewayCertificateKey,
  gatewayCertificateLabel,
  gatewayDeploymentSummary,
  gatewaySyncError,
  isExpired,
  isExpiringSoon,
  libraryCoverage,
  manualUploadConfigured,
  manualUploadSummary,
  multiSniPreview,
  primaryCertificateBadgeLabel,
  recommendedCertificateId,
  showMultiSniSuggestion,
  singleActivePreview,
  sourceLabel,
  statusOverviewText,
  subdomainCoverage,
  uncoveredHostsPreview,
} = useCertConfigViewModel({
  formData,
  locale,
  sslStatus,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

onMounted(() => {
  void loadSSLStatus();
});

async function loadSSLStatus() {
  await runLoadSSLStatus(async () => {
    sslStatus.value = await ConfigAPI.getSSLStatus();
  });
  hasLoadedSSLStatus.value = true;
}

async function handleSave(activate: boolean) {
  pendingSaveMode.value = activate ? "activate" : "store";
  errorMessage.value = "";
  await runSaveSSL(async () => {
    await ConfigAPI.setSSL({
      label: t("admin.certConfig.manualCertificateLabel"),
      source: "manual",
      cert: formData.value.cert,
      key: formData.value.key,
      activate,
    });
    formData.value = { cert: "", key: "" };
    await loadSSLStatus();
    toast.success(
      activate
        ? t("admin.certConfig.saveAndActivateSuccess")
        : t("admin.certConfig.saveToLibrarySuccess"),
    );
  });
  pendingSaveMode.value = null;
}

function resetManualUploadForm() {
  formData.value = { cert: "", key: "" };
  errorMessage.value = "";
}

async function handleClear() {
  await runClearSSL(async () => {
    await ConfigAPI.deleteSSL();
    await loadSSLStatus();
    toast.success(t("admin.certConfig.disableSuccess"));
  });
}

async function activateCertificate(id: string) {
  activatingCertificateId.value = id;
  await runActivateSSL(async () => {
    await ConfigAPI.activateSSLCertificate(id);
    await loadSSLStatus();
    toast.success(
      sslStatus.value?.deploymentMode === "multi_sni"
        ? t("admin.certConfig.defaultCertificateSwitched")
        : t("admin.certConfig.activeCertificateSwitched"),
    );
  });
  activatingCertificateId.value = null;
}

async function activateRecommendedCertificate() {
  if (!recommendedCertificateId.value) return;
  await activateCertificate(recommendedCertificateId.value);
}

async function updateDeploymentMode(mode: "single_active" | "multi_sni") {
  if (!sslStatus.value || sslStatus.value.deploymentMode === mode) return;
  pendingDeploymentMode.value = mode;
  await runUpdateDeploymentMode(async () => {
    sslStatus.value = await ConfigAPI.updateSSLDeploymentMode(mode);
    toast.success(
      mode === "multi_sni"
        ? t("admin.certConfig.switchedToMultiSni")
        : t("admin.certConfig.switchedToSingleActive"),
    );
  });
  pendingDeploymentMode.value = null;
}

async function deleteCertificate(id: string) {
  deletingCertificateId.value = id;
  await runDeleteSSL(async () => {
    await ConfigAPI.deleteSSLCertificate(id);
    await loadSSLStatus();
    toast.success(t("admin.certConfig.deleteSuccess"));
  });
  deletingCertificateId.value = null;
}

async function handleClearLibrary() {
  await runClearSSLLibrary(async () => {
    await ConfigAPI.clearSSLCertificateLibrary();
    await loadSSLStatus();
    toast.success(t("admin.certConfig.clearLibrarySuccess"));
  });
}
</script>
