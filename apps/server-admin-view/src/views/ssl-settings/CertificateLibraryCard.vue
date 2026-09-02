<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  ChevronUp,
  Download,
  Loader2,
  ShieldCheck,
  Trash2,
} from "lucide-vue-next";
import type {
  SSLCertificateSource,
  SSLCertificateSummary,
  SubdomainCertificateCoverage,
} from "@/types";

defineProps<{
  activateButtonLabel: string;
  activateCertificate: (id: string) => Promise<void> | void;
  activatingCertificateId: string | null;
  certificateDisplayLabel: (certificate: SSLCertificateSummary) => string;
  certificateRoleLabel: (certificate: SSLCertificateSummary) => string;
  certificates: SSLCertificateSummary[];
  clearLibrary: () => Promise<void> | void;
  coverageBadgeClass: (coverage: SubdomainCertificateCoverage) => string;
  coverageBadgeLabel: (coverage: SubdomainCertificateCoverage) => string;
  coverageBadgeVariant: (
    coverage: SubdomainCertificateCoverage,
  ) => "default" | "destructive" | "outline" | "secondary";
  deleteCertificate: (id: string) => Promise<void> | void;
  deletingCertificateId: string | null;
  downloadCertificate: (
    certificate: SSLCertificateSummary,
  ) => Promise<void> | void;
  downloadingCertificateId: string | null;
  formatDate: (date: string) => string;
  isActivating: boolean;
  isClearingLibrary: boolean;
  isDeleting: boolean;
  isDownloading: boolean;
  isMutationPending: boolean;
  ready: boolean;
  sourceLabel: (source: SSLCertificateSource) => string;
  summary: string;
}>();

const { t } = useI18n();
</script>

<template>
  <TooltipProvider>
    <ConfigCollapsibleCard
      v-if="certificates.length"
      :title="t('admin.certConfig.libraryTitle')"
      :configured="true"
      :ready="ready"
      :edit-label="t('admin.certConfig.viewCertificates')"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-transparent px-4 py-3 sm:px-6 flex flex-wrap items-center justify-end gap-1 rounded-b-lg"
      card-class="dynamic-white-cert-card"
    >
      <template #summary>
        {{ summary }}
      </template>

      <template #default>
        <div class="p-4 sm:p-6 grid gap-3 xl:grid-cols-2">
          <div
            v-for="certificate in certificates"
            :key="certificate.id"
            class="dynamic-white-cert-subsurface grid gap-3 rounded-lg border bg-muted/10 p-3.5"
          >
            <div class="flex flex-wrap items-start justify-between gap-3">
              <div class="grid min-w-0 flex-1 gap-1">
                <div class="flex flex-wrap items-center gap-2">
                  <div class="font-medium break-all">
                    {{ certificateDisplayLabel(certificate) }}
                  </div>
                  <Badge
                    v-if="certificate.is_active"
                    variant="default"
                    class="dynamic-white-glass-chip dynamic-white-glass-chip-success bg-green-600 hover:bg-green-600"
                  >
                    {{ certificateRoleLabel(certificate) }}
                  </Badge>
                  <Badge variant="outline">
                    {{ sourceLabel(certificate.source) }}
                  </Badge>
                  <Badge
                    v-if="certificate.coverage"
                    :variant="coverageBadgeVariant(certificate.coverage)"
                    :class="coverageBadgeClass(certificate.coverage)"
                  >
                    {{ coverageBadgeLabel(certificate.coverage) }}
                  </Badge>
                </div>
                <div class="text-xs text-muted-foreground font-mono break-all">
                  {{
                    certificate.certInfo?.dnsNames?.join(", ") ||
                    certificate.primary_domain ||
                    t("admin.certConfig.noDomainInfo")
                  }}
                </div>
              </div>

              <div class="flex shrink-0 items-center gap-0.5">
                <Tooltip v-if="!certificate.is_active">
                  <TooltipTrigger as-child>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      :aria-label="activateButtonLabel"
                      :disabled="isMutationPending"
                      @click="activateCertificate(certificate.id)"
                    >
                      <Loader2
                        v-if="
                          isActivating &&
                          activatingCertificateId === certificate.id
                        "
                        class="h-4 w-4 animate-spin"
                      />
                      <ShieldCheck v-else class="h-4 w-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{{ activateButtonLabel }}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      :aria-label="t('admin.certConfig.download')"
                      :disabled="isDownloading || isMutationPending"
                      @click="downloadCertificate(certificate)"
                    >
                      <Loader2
                        v-if="
                          isDownloading &&
                          downloadingCertificateId === certificate.id
                        "
                        class="h-4 w-4 animate-spin"
                      />
                      <Download v-else class="h-4 w-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    {{ t("admin.certConfig.download") }}
                  </TooltipContent>
                </Tooltip>
                <ConfirmDangerPopover
                  :title="t('admin.certConfig.deleteTitle')"
                  :description="t('admin.certConfig.deleteDescription')"
                  :confirm-text="t('admin.certConfig.deleteConfirm')"
                  :loading="
                    isDeleting && deletingCertificateId === certificate.id
                  "
                  :disabled="isMutationPending"
                  :on-confirm="() => deleteCertificate(certificate.id)"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      :aria-label="t('admin.certConfig.delete')"
                      :title="t('admin.certConfig.delete')"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                      :disabled="isMutationPending"
                    >
                      <Loader2
                        v-if="
                          isDeleting && deletingCertificateId === certificate.id
                        "
                        class="h-4 w-4 animate-spin"
                      />
                      <Trash2 v-else class="h-4 w-4" />
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </div>

            <div class="grid gap-2 text-xs text-muted-foreground">
              <div
                v-if="
                  certificate.certInfo?.validFrom ||
                  certificate.certInfo?.validTo
                "
              >
                {{ t("admin.certConfig.validityLabel") }}
                {{ formatDate(certificate.certInfo?.validFrom || "") }}
                <span class="mx-1">{{ t("admin.certConfig.to") }}</span>
                {{ formatDate(certificate.certInfo?.validTo || "") }}
              </div>
              <div>
                {{
                  t("admin.certConfig.updatedAtLabel", {
                    value: formatDate(certificate.updated_at),
                  })
                }}
              </div>
              <div v-if="certificate.coverage?.summary">
                {{ certificate.coverage.summary }}
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <ConfirmDangerPopover
          :title="t('admin.certConfig.clearLibraryTitle')"
          :description="t('admin.certConfig.clearLibraryDescription')"
          :confirm-text="t('admin.certConfig.clearLibraryConfirm')"
          :loading="isClearingLibrary"
          :disabled="isMutationPending"
          :on-confirm="clearLibrary"
        >
          <template #trigger>
            <Button
              variant="destructive-outline"
              size="sm"
              :disabled="isMutationPending"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("admin.certConfig.clearLibrary") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="t('admin.certConfig.collapse')"
              @click="collapse"
            >
              <ChevronUp class="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{{ t("admin.certConfig.collapse") }}</TooltipContent>
        </Tooltip>
      </template>
    </ConfigCollapsibleCard>
  </TooltipProvider>
</template>
