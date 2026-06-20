<template>
  <Card
    v-if="
      !hasLoadedSSLStatus || (isLoading && showLoadingSkeleton && !sslStatus)
    "
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
      <div class="rounded-lg border bg-muted/30 p-4 grid gap-3">
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
    <Card class="overflow-hidden">
      <CardHeader>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="grid gap-1">
            <CardTitle class="flex items-center gap-3">
              <span>{{ t("admin.certConfig.currentStatus") }}</span>
              <Badge
                :variant="activeCertificate ? 'default' : 'secondary'"
                :class="
                  activeCertificate ? 'bg-green-600 hover:bg-green-600' : ''
                "
              >
                {{ primaryCertificateBadgeLabel }}
              </Badge>
            </CardTitle>
            <CardDescription class="leading-6">
              {{ statusOverviewText }}
            </CardDescription>
          </div>
          <div class="flex flex-wrap gap-2">
            <Badge variant="outline">
              {{ deploymentModeLabel }}
            </Badge>
            <Badge variant="secondary">
              {{
                t("admin.certConfig.certificateLibraryCount", {
                  count: certificates.length,
                })
              }}
            </Badge>
          </div>
        </div>
      </CardHeader>

      <CardContent v-if="libraryCoverage" class="pt-0">
        <Alert
          :variant="
            libraryCoverage.status === 'missing' ? 'destructive' : 'default'
          "
        >
          <AlertTitle>{{ t("admin.certConfig.subdomainLoopTitle") }}</AlertTitle>
          <AlertDescription class="grid gap-2">
            <p>{{ libraryCoverage.summary }}</p>
            <p
              v-if="
                libraryCoverage.combined_covering_certificate_ids.length > 1
              "
              class="text-xs text-muted-foreground"
            >
              {{
                t("admin.certConfig.combinedCoverageCount", {
                  count:
                    libraryCoverage.combined_covering_certificate_ids.length,
                })
              }}
            </p>
            <div
              v-if="libraryCoverage.warnings.length"
              class="grid gap-1 text-xs text-muted-foreground"
            >
              <div v-for="warning in libraryCoverage.warnings" :key="warning">
                {{ warning }}
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <Button
                v-if="showMultiSniSuggestion"
                size="sm"
                variant="outline"
                :disabled="isUpdatingDeploymentMode"
                @click="updateDeploymentMode('multi_sni')"
              >
                {{ t("admin.certConfig.switchToMultiSni") }}
              </Button>
              <Button
                v-if="recommendedCertificateId"
                size="sm"
                :disabled="isActivating"
                @click="activateRecommendedCertificate"
              >
                {{ t("admin.certConfig.switchToRecommended") }}
              </Button>
            </div>
          </AlertDescription>
        </Alert>
      </CardContent>

      <CardFooter
        v-if="activeCertificate"
        class="flex flex-wrap justify-end gap-2 border-t pt-6"
      >
        <ConfirmDangerPopover
          :title="t('admin.certConfig.disableTitle')"
          :description="t('admin.certConfig.disableDescription')"
          :confirm-text="t('admin.certConfig.disableConfirm')"
          :loading="isClearing"
          :disabled="isClearing"
          :on-confirm="handleClear"
        >
          <template #trigger>
            <Button variant="destructive" size="sm" :disabled="isClearing">
              {{ t("admin.certConfig.disableHttps") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </CardFooter>
    </Card>

    <ConfigCollapsibleCard
      :title="t('admin.certConfig.deploymentTitle')"
      :configured="deploymentSectionConfigured"
      :ready="hasLoadedSSLStatus"
      :edit-label="t('admin.certConfig.viewConfig')"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        {{ deploymentSummary }}
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6 grid gap-2">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div class="text-base font-semibold">
                {{ t("admin.certConfig.deploymentHeading") }}
              </div>
              <Badge variant="outline">{{ deploymentModeShortLabel }}</Badge>
            </div>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.certConfig.deploymentIntro") }}
            </p>
            <p class="text-xs text-muted-foreground">
              {{ deploymentModeDescription }}
            </p>
            <p v-if="deploymentModeMismatch" class="text-xs text-amber-600">
              {{
                t("admin.certConfig.deploymentMismatch", {
                  configured: configuredDeploymentModeLabel,
                  current: deploymentModeShortLabel,
                })
              }}
            </p>
            <p v-else-if="gatewaySyncError" class="text-xs text-amber-600">
              {{ gatewaySyncError }}
            </p>
          </div>

          <div class="grid gap-3 p-4 sm:p-6 lg:grid-cols-2">
            <div
              class="rounded-lg border p-4 grid gap-3 transition-colors"
              :class="deploymentCardClass('single_active')"
            >
              <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="grid gap-1">
                  <div class="text-sm font-medium">
                    {{ t("admin.certConfig.singleActiveTitle") }}
                  </div>
                  <p class="text-xs text-muted-foreground">
                    {{ t("admin.certConfig.singleActiveDescription") }}
                  </p>
                </div>
                <Badge
                  v-if="sslStatus?.deploymentMode === 'single_active'"
                  variant="default"
                  class="bg-green-600 hover:bg-green-600"
                >
                  {{ t("admin.certConfig.currentMode") }}
                </Badge>
              </div>

              <div class="grid gap-2 text-xs text-muted-foreground">
                <div>
                  {{
                    t("admin.certConfig.expectedDeploy", {
                      count: singleActivePreview.count,
                    })
                  }}
                </div>
                <div>
                  {{
                    t("admin.certConfig.publicCertificate", {
                      label: singleActivePreview.defaultLabel,
                    })
                  }}
                </div>
                <div v-if="singleActivePreview.domainSummary">
                  {{
                    t("admin.certConfig.coveredDomains", {
                      domains: singleActivePreview.domainSummary,
                    })
                  }}
                </div>
              </div>

              <Button
                variant="outline"
                class="justify-start"
                :disabled="
                  isUpdatingDeploymentMode ||
                  sslStatus?.deploymentMode === 'single_active'
                "
                @click="updateDeploymentMode('single_active')"
              >
                <span
                  v-if="
                    isUpdatingDeploymentMode &&
                    pendingDeploymentMode === 'single_active'
                  "
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
                ></span>
                {{
                  sslStatus?.deploymentMode === "single_active"
                    ? t("admin.certConfig.currentlyInUse")
                    : t("admin.certConfig.switchToSingleActive")
                }}
              </Button>
            </div>

            <div
              class="rounded-lg border p-4 grid gap-3 transition-colors"
              :class="deploymentCardClass('multi_sni')"
            >
              <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="grid gap-1">
                  <div class="text-sm font-medium">
                    {{ t("admin.certConfig.multiSniTitle") }}
                  </div>
                  <p class="text-xs text-muted-foreground">
                    {{ t("admin.certConfig.multiSniDescription") }}
                  </p>
                </div>
                <Badge
                  v-if="sslStatus?.deploymentMode === 'multi_sni'"
                  variant="default"
                  class="bg-green-600 hover:bg-green-600"
                >
                  {{ t("admin.certConfig.currentMode") }}
                </Badge>
              </div>

              <div class="grid gap-2 text-xs text-muted-foreground">
                <div>
                  {{
                    t("admin.certConfig.expectedDeploy", {
                      count: multiSniPreview.count,
                    })
                  }}
                </div>
                <div>
                  {{
                    t("admin.certConfig.defaultCertificate", {
                      label: multiSniPreview.defaultLabel,
                    })
                  }}
                </div>
                <div
                  v-if="multiSniPreview.previewItems.length"
                  class="flex flex-wrap gap-1.5"
                >
                  <Badge
                    v-for="item in multiSniPreview.previewItems"
                    :key="item.id"
                    variant="secondary"
                    class="max-w-full"
                  >
                    <span class="truncate">{{ item.label }}</span>
                    <span
                      v-if="item.isDefault"
                      class="ml-1 text-[10px] text-muted-foreground"
                      >{{ t("admin.certConfig.defaultTag") }}</span
                    >
                  </Badge>
                  <Badge
                    v-if="multiSniPreview.remainingCount > 0"
                    variant="outline"
                  >
                    {{
                      t("admin.certConfig.moreCertificates", {
                        count: multiSniPreview.remainingCount,
                      })
                    }}
                  </Badge>
                </div>
              </div>

              <Button
                class="justify-start"
                :disabled="
                  isUpdatingDeploymentMode ||
                  !certificates.length ||
                  sslStatus?.deploymentMode === 'multi_sni'
                "
                @click="updateDeploymentMode('multi_sni')"
              >
                <span
                  v-if="
                    isUpdatingDeploymentMode &&
                    pendingDeploymentMode === 'multi_sni'
                  "
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                ></span>
                {{
                  sslStatus?.deploymentMode === "multi_sni"
                    ? t("admin.certConfig.currentlyInUse")
                    : t("admin.certConfig.switchToMultiSni")
                }}
              </Button>
            </div>
          </div>

          <div class="p-4 sm:p-6">
            <div
              class="rounded-lg border border-dashed bg-muted/20 p-4 grid gap-2"
            >
              <div class="text-xs font-medium">
                {{ t("admin.certConfig.gatewayReceivedTitle") }}
              </div>
              <p class="text-xs text-muted-foreground">
                {{ gatewayDeploymentSummary }}
              </p>
              <div
                v-if="deployedGatewayCertificates.length"
                class="flex flex-wrap gap-1.5"
              >
                <Badge
                  v-for="certificate in deployedGatewayCertificates"
                  :key="gatewayCertificateKey(certificate)"
                  variant="secondary"
                  class="max-w-full"
                >
                  <span class="truncate">{{
                    gatewayCertificateLabel(certificate)
                  }}</span>
                  <span
                    v-if="certificate.is_default"
                    class="ml-1 text-[10px] text-muted-foreground"
                  >
                    {{ t("admin.certConfig.defaultTag") }}
                  </span>
                </Badge>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">
          {{ t("admin.certConfig.collapse") }}
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <ConfigCollapsibleCard
      v-if="activeCertificate || subdomainCoverage"
      :title="t('admin.certConfig.currentCertificateTitle')"
      :configured="Boolean(activeCertificate?.certInfo)"
      :ready="hasLoadedSSLStatus"
      :edit-label="t('common.viewDetails')"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col gap-2 rounded-b-lg sm:flex-row sm:justify-end"
    >
      <template #summary>
        {{ currentCertificateSummary }}
      </template>

      <template #default>
        <div class="p-4 sm:p-6">
          <div
            v-if="activeCertificate?.certInfo"
            class="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]"
          >
            <div class="rounded-lg border bg-muted/20 p-4 grid gap-4">
              <div
                class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
              >
                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldName") }}
                </span>
                <span class="min-w-0 font-medium">{{
                  activeCertificate.label
                }}</span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldSource") }}
                </span>
                <span class="min-w-0 text-xs">{{
                  sourceLabel(activeCertificate.source)
                }}</span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldIssuer") }}
                </span>
                <span class="min-w-0 font-mono text-xs break-all">{{
                  formatDN(activeCertificate.certInfo.issuer)
                }}</span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldSubject") }}
                </span>
                <span class="min-w-0 font-mono text-xs break-all">{{
                  formatDN(activeCertificate.certInfo.subject)
                }}</span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldValidity") }}
                </span>
                <span class="min-w-0 text-xs">
                  <span>{{
                    formatDate(activeCertificate.certInfo.validFrom)
                  }}</span>
                  <span class="mx-1 text-muted-foreground">
                    {{ t("admin.certConfig.to") }}
                  </span>
                  <span
                    :class="isExpired ? 'text-destructive font-semibold' : ''"
                  >
                    {{ formatDate(activeCertificate.certInfo.validTo) }}
                  </span>
                  <Badge
                    v-if="isExpired"
                    variant="destructive"
                    class="ml-2 text-[10px]"
                    >{{ t("admin.certConfig.expired") }}</Badge
                  >
                  <Badge
                    v-else-if="isExpiringSoon"
                    variant="outline"
                    class="ml-2 text-[10px] border-yellow-500 text-yellow-600"
                  >
                    {{ t("admin.certConfig.expiringSoon") }}
                  </Badge>
                </span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldDomains") }}
                </span>
                <div class="min-w-0 flex flex-wrap gap-1.5">
                  <Badge
                    v-for="dns in activeCertificate.certInfo.dnsNames"
                    :key="dns"
                    variant="secondary"
                    class="font-mono text-xs"
                  >
                    {{ dns }}
                  </Badge>
                  <span
                    v-if="!activeCertificate.certInfo.dnsNames.length"
                    class="text-xs text-muted-foreground"
                  >
                    {{ t("admin.certConfig.none") }}
                  </span>
                </div>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.fieldUpdatedAt") }}
                </span>
                <span class="min-w-0 text-xs text-muted-foreground">
                  {{ formatDate(activeCertificate.updated_at) }}
                </span>
              </div>
            </div>

            <div
              v-if="subdomainCoverage"
              class="rounded-lg border bg-background/80 p-4 grid gap-3"
            >
              <div class="flex flex-wrap items-center justify-between gap-3">
                <div class="text-sm font-medium">
                  {{ t("admin.certConfig.coverageAnalysisTitle") }}
                </div>
                <Badge
                  :variant="coverageBadgeVariant(subdomainCoverage)"
                  :class="coverageBadgeClass(subdomainCoverage)"
                >
                  {{ coverageBadgeLabel(subdomainCoverage) }}
                </Badge>
              </div>
              <p class="text-sm text-muted-foreground">
                {{ subdomainCoverage.summary }}
              </p>
              <div
                class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
              >
                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.authService") }}
                </span>
                <span class="min-w-0 font-mono text-xs break-all">
                  {{
                    subdomainCoverage.auth_host ||
                    t("admin.certConfig.notConfigured")
                  }}
                </span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.recommendedDomains") }}
                </span>
                <span class="min-w-0 font-mono text-xs break-all">
                  {{
                    subdomainCoverage.recommended_domains.length
                      ? subdomainCoverage.recommended_domains.join(", ")
                      : t("admin.certConfig.noRecommendation")
                  }}
                </span>

                <span class="text-muted-foreground font-medium">
                  {{ t("admin.certConfig.hostCoverage") }}
                </span>
                <span class="min-w-0 text-xs">
                  {{ subdomainCoverage.covered_hosts.length }} /
                  {{
                    subdomainCoverage.covered_hosts.length +
                    subdomainCoverage.uncovered_hosts.length
                  }}
                  {{
                    t("admin.certConfig.hostCoverageCount", {
                      covered: subdomainCoverage.covered_hosts.length,
                      total:
                        subdomainCoverage.covered_hosts.length +
                        subdomainCoverage.uncovered_hosts.length,
                    })
                  }}
                </span>
              </div>
              <div
                v-if="subdomainCoverage.uncovered_hosts.length"
                class="text-xs text-amber-600"
              >
                {{
                  t("admin.certConfig.uncoveredHosts", {
                    hosts: uncoveredHostsPreview(
                      subdomainCoverage.uncovered_hosts,
                    ),
                  })
                }}
              </div>
              <div
                v-if="subdomainCoverage.warnings.length"
                class="grid gap-1 text-xs text-muted-foreground"
              >
                <div
                  v-for="warning in subdomainCoverage.warnings"
                  :key="warning"
                >
                  {{ warning }}
                </div>
              </div>
            </div>
          </div>

          <Alert v-else variant="default">
            <AlertTitle>{{ t("admin.certConfig.noActiveTitle") }}</AlertTitle>
            <AlertDescription class="grid gap-2">
              <p>{{ t("admin.certConfig.noActiveDescription") }}</p>
              <p v-if="subdomainCoverage" class="text-xs text-muted-foreground">
                {{ subdomainCoverage.summary }}
              </p>
            </AlertDescription>
          </Alert>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">
          {{ t("admin.certConfig.collapse") }}
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <ConfigCollapsibleCard
      :title="t('admin.certConfig.manualUploadTitle')"
      :configured="manualUploadConfigured"
      :ready="hasLoadedSSLStatus"
      :edit-label="t('admin.certConfig.expandForm')"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        {{ manualUploadSummary }}
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6 grid gap-2">
            <div class="text-base font-semibold">
              {{ t("admin.certConfig.uploadNewTitle") }}
            </div>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.certConfig.uploadDescription") }}
            </p>
          </div>

          <div class="p-4 sm:p-6 grid gap-6">
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
              <AlertTitle>
                {{ t("admin.certConfig.validationFailed") }}
              </AlertTitle>
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
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import CertForm from "@admin-shared/components/ssl/CertForm.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../../lib/api";
import type { SSLStatus } from "../../types";
import { toast } from "@admin-shared/utils/toast";
import CertificateLibraryCard from "./CertificateLibraryCard.vue";
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
    toast.error(extractErrorMessage(error, t("admin.certConfig.disableFailed")));
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
    toast.error(
      extractErrorMessage(error, t("admin.certConfig.deleteFailed")),
    );
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
  loadSSLStatus();
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
