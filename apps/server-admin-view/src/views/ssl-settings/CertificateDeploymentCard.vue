<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { SSLDeploymentMode, SSLStatus } from "@/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import type {
  DeploymentPreviewItem,
  GatewayCertificateItem,
} from "./useCertConfigViewModel";

type DeploymentPreview = {
  count: number;
  defaultLabel: string;
  domainSummary: string;
  previewItems: DeploymentPreviewItem[];
  remainingCount: number;
};

defineProps<{
  certificateCount: number;
  configuredDeploymentModeLabel: string;
  deployedGatewayCertificates: GatewayCertificateItem[];
  deploymentCardClass: (mode: SSLDeploymentMode) => string;
  deploymentModeDescription: string;
  deploymentModeMismatch: boolean;
  deploymentModeShortLabel: string;
  deploymentSectionConfigured: boolean;
  deploymentSummary: string;
  gatewayCertificateKey: (certificate: GatewayCertificateItem) => string;
  gatewayCertificateLabel: (certificate: GatewayCertificateItem) => string;
  gatewayDeploymentSummary: string;
  gatewaySyncError: string;
  isUpdatingDeploymentMode: boolean;
  multiSniPreview: DeploymentPreview;
  pendingDeploymentMode: SSLDeploymentMode | null;
  ready: boolean;
  singleActivePreview: DeploymentPreview;
  sslStatus: SSLStatus | null;
}>();

const emit = defineEmits<{
  updateMode: [mode: SSLDeploymentMode];
}>();
const { t } = useI18n();
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.certConfig.deploymentTitle')"
    :configured="deploymentSectionConfigured"
    :ready="ready"
    :edit-label="t('admin.certConfig.viewConfig')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    card-class="dynamic-white-cert-card"
  >
    <template #summary>{{ deploymentSummary }}</template>

    <template #default>
      <div class="divide-y divide-border">
        <div class="grid gap-2 p-4 sm:p-6">
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
          <p
            v-if="deploymentModeMismatch"
            class="text-xs text-amber-600 dark:text-amber-400"
          >
            {{
              t("admin.certConfig.deploymentMismatch", {
                configured: configuredDeploymentModeLabel,
                current: deploymentModeShortLabel,
              })
            }}
          </p>
          <p
            v-else-if="gatewaySyncError"
            class="text-xs text-amber-600 dark:text-amber-400"
          >
            {{ gatewaySyncError }}
          </p>
        </div>

        <div class="grid gap-3 p-4 sm:p-6 lg:grid-cols-2">
          <div
            class="dynamic-white-cert-subsurface grid gap-3 rounded-lg border p-4 transition-colors"
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
              :class="
                sslStatus?.deploymentMode === 'single_active'
                  ? 'border-border/70 bg-muted/60 text-foreground disabled:opacity-100 dark:bg-muted/40'
                  : ''
              "
              :disabled="
                isUpdatingDeploymentMode ||
                sslStatus?.deploymentMode === 'single_active'
              "
              @click="emit('updateMode', 'single_active')"
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
            class="dynamic-white-cert-subsurface grid gap-3 rounded-lg border p-4 transition-colors"
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
                  >
                    {{ t("admin.certConfig.defaultTag") }}
                  </span>
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
              :class="
                sslStatus?.deploymentMode === 'multi_sni'
                  ? 'border border-border/70 bg-muted/60 text-foreground hover:bg-muted/60 disabled:opacity-100 dark:bg-muted/40 dark:hover:bg-muted/40'
                  : ''
              "
              :disabled="
                isUpdatingDeploymentMode ||
                !certificateCount ||
                sslStatus?.deploymentMode === 'multi_sni'
              "
              @click="emit('updateMode', 'multi_sni')"
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
            class="dynamic-white-cert-subsurface grid gap-2 rounded-lg border border-dashed bg-muted/20 p-4"
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
                <span class="truncate">
                  {{ gatewayCertificateLabel(certificate) }}
                </span>
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
</template>
