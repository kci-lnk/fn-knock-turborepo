<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
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
  formatDate: (date: string) => string;
  isActivating: boolean;
  isClearingLibrary: boolean;
  isDeleting: boolean;
  ready: boolean;
  sourceLabel: (source: SSLCertificateSource) => string;
  summary: string;
}>();

const { t } = useI18n();
</script>

<template>
  <ConfigCollapsibleCard
    v-if="certificates.length"
    :title="t('admin.certConfig.libraryTitle')"
    :configured="true"
    :ready="ready"
    :edit-label="t('admin.certConfig.viewCertificates')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col gap-2 rounded-b-lg sm:flex-row sm:justify-end"
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
          class="dynamic-white-cert-subsurface rounded-lg border bg-muted/15 p-4 grid gap-3"
        >
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="grid gap-1 min-w-0">
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

            <div class="flex flex-wrap gap-2">
              <Button
                v-if="!certificate.is_active"
                size="sm"
                :disabled="isActivating"
                @click="activateCertificate(certificate.id)"
              >
                <span
                  v-if="
                    isActivating && activatingCertificateId === certificate.id
                  "
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                ></span>
                {{ activateButtonLabel }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.certConfig.deleteTitle')"
                :description="t('admin.certConfig.deleteDescription')"
                :confirm-text="t('admin.certConfig.deleteConfirm')"
                :loading="
                  isDeleting && deletingCertificateId === certificate.id
                "
                :disabled="
                  isDeleting && deletingCertificateId === certificate.id
                "
                :on-confirm="() => deleteCertificate(certificate.id)"
              >
                <template #trigger>
                  <Button
                    variant="destructive"
                    size="sm"
                    :disabled="
                      isDeleting && deletingCertificateId === certificate.id
                    "
                  >
                    {{ t("admin.certConfig.delete") }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </div>

          <div class="grid gap-2 text-xs text-muted-foreground">
            <div>
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
        :disabled="isClearingLibrary"
        :on-confirm="clearLibrary"
      >
        <template #trigger>
          <Button variant="destructive" :disabled="isClearingLibrary">
            {{ t("admin.certConfig.clearLibrary") }}
          </Button>
        </template>
      </ConfirmDangerPopover>
      <Button variant="outline" @click="collapse">
        {{ t("admin.certConfig.collapse") }}
      </Button>
    </template>
  </ConfigCollapsibleCard>
</template>
