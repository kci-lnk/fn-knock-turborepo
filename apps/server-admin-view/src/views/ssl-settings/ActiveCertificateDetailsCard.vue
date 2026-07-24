<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type {
  SSLCertificateSummary,
  SubdomainCertificateCoverage,
} from "@/types";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";

defineProps<{
  activeCertificate: SSLCertificateSummary | null;
  coverageBadgeClass: (coverage: SubdomainCertificateCoverage) => string;
  coverageBadgeLabel: (coverage: SubdomainCertificateCoverage) => string;
  coverageBadgeVariant: (
    coverage: SubdomainCertificateCoverage,
  ) => "default" | "destructive" | "outline";
  currentCertificateSummary: string;
  formatDate: (value: string) => string;
  formatDn: (value: string) => string;
  isExpired: boolean;
  isExpiringSoon: boolean;
  ready: boolean;
  sourceLabel: (source: SSLCertificateSummary["source"]) => string;
  subdomainCoverage: SubdomainCertificateCoverage | null;
  uncoveredHostsPreview: (hosts: string[]) => string;
}>();

const { t } = useI18n();
</script>

<template>
  <ConfigCollapsibleCard
    v-if="activeCertificate || subdomainCoverage"
    :title="t('admin.certConfig.currentCertificateTitle')"
    :configured="Boolean(activeCertificate?.certInfo)"
    :ready="ready"
    :edit-label="t('common.viewDetails')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col gap-2 rounded-b-lg sm:flex-row sm:justify-end"
    card-class="dynamic-white-cert-card"
  >
    <template #summary>{{ currentCertificateSummary }}</template>

    <template #default>
      <div class="p-4 sm:p-6">
        <div
          v-if="activeCertificate?.certInfo"
          class="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]"
        >
          <div
            class="dynamic-white-cert-subsurface grid gap-4 rounded-lg border bg-muted/20 p-4"
          >
            <div
              class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
            >
              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.fieldName") }}
              </span>
              <span class="min-w-0 font-medium">
                {{ activeCertificate.label }}
              </span>

              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.fieldSource") }}
              </span>
              <span class="min-w-0 text-xs">
                {{ sourceLabel(activeCertificate.source) }}
              </span>

              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.fieldIssuer") }}
              </span>
              <span class="min-w-0 break-all font-mono text-xs">
                {{ formatDn(activeCertificate.certInfo.issuer) }}
              </span>

              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.fieldSubject") }}
              </span>
              <span class="min-w-0 break-all font-mono text-xs">
                {{ formatDn(activeCertificate.certInfo.subject) }}
              </span>

              <span class="font-medium text-muted-foreground">
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
                  :class="isExpired ? 'font-semibold text-destructive' : ''"
                >
                  {{ formatDate(activeCertificate.certInfo.validTo) }}
                </span>
                <Badge
                  v-if="isExpired"
                  variant="destructive"
                  class="ml-2 text-[10px]"
                >
                  {{ t("admin.certConfig.expired") }}
                </Badge>
                <Badge
                  v-else-if="isExpiringSoon"
                  variant="outline"
                  class="ml-2 border-yellow-500 text-[10px] text-yellow-600 dark:border-yellow-400/80 dark:text-yellow-300"
                >
                  {{ t("admin.certConfig.expiringSoon") }}
                </Badge>
              </span>

              <span class="font-medium text-muted-foreground">
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

              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.fieldUpdatedAt") }}
              </span>
              <span class="min-w-0 text-xs text-muted-foreground">
                {{ formatDate(activeCertificate.updated_at) }}
              </span>
            </div>
          </div>

          <div
            v-if="subdomainCoverage"
            class="dynamic-white-cert-subsurface grid gap-3 rounded-lg border bg-background/80 p-4"
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
              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.authService") }}
              </span>
              <span class="min-w-0 break-all font-mono text-xs">
                {{
                  subdomainCoverage.auth_host ||
                  t("admin.certConfig.notConfigured")
                }}
              </span>

              <span class="font-medium text-muted-foreground">
                {{ t("admin.certConfig.recommendedDomains") }}
              </span>
              <span class="min-w-0 break-all font-mono text-xs">
                {{
                  subdomainCoverage.recommended_domains.length
                    ? subdomainCoverage.recommended_domains.join(", ")
                    : t("admin.certConfig.noRecommendation")
                }}
              </span>

              <span class="font-medium text-muted-foreground">
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
              class="text-xs text-amber-600 dark:text-amber-400"
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
              <div v-for="warning in subdomainCoverage.warnings" :key="warning">
                {{ warning }}
              </div>
            </div>
          </div>
        </div>

        <Alert v-else variant="default" class="dynamic-white-glass-surface">
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
</template>
