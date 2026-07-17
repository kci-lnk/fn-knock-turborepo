<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type {
  SSLCertificateSummary,
  SubdomainCertificateLibraryCoverage,
} from "@/types";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";

defineProps<{
  activeCertificate: SSLCertificateSummary | null;
  certificateCount: number;
  deploymentModeLabel: string;
  isActivating: boolean;
  isClearing: boolean;
  isUpdatingDeploymentMode: boolean;
  libraryCoverage: SubdomainCertificateLibraryCoverage | null;
  primaryCertificateBadgeLabel: string;
  recommendedCertificateId: string;
  showMultiSniSuggestion: boolean;
  statusOverviewText: string;
}>();

const emit = defineEmits<{
  activateRecommended: [];
  clear: [];
  switchToMultiSni: [];
}>();
const { t } = useI18n();
</script>

<template>
  <Card class="dynamic-white-cert-card">
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
          <Badge variant="outline">{{ deploymentModeLabel }}</Badge>
          <Badge variant="secondary">
            {{
              t("admin.certConfig.certificateLibraryCount", {
                count: certificateCount,
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
        class="dynamic-white-glass-surface"
      >
        <AlertTitle>{{ t("admin.certConfig.subdomainLoopTitle") }}</AlertTitle>
        <AlertDescription class="grid gap-2">
          <p>{{ libraryCoverage.summary }}</p>
          <p
            v-if="libraryCoverage.combined_covering_certificate_ids.length > 1"
            class="text-xs text-muted-foreground"
          >
            {{
              t("admin.certConfig.combinedCoverageCount", {
                count: libraryCoverage.combined_covering_certificate_ids.length,
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
              @click="emit('switchToMultiSni')"
            >
              {{ t("admin.certConfig.switchToMultiSni") }}
            </Button>
            <Button
              v-if="recommendedCertificateId"
              size="sm"
              :disabled="isActivating"
              @click="emit('activateRecommended')"
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
        :on-confirm="() => emit('clear')"
      >
        <template #trigger>
          <Button variant="destructive" size="sm" :disabled="isClearing">
            {{ t("admin.certConfig.disableHttps") }}
          </Button>
        </template>
      </ConfirmDangerPopover>
    </CardFooter>
  </Card>
</template>
