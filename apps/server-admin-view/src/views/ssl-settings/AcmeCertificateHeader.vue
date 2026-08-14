<script setup lang="ts">
import { AlertTriangle } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import RefreshButton from "@/components/RefreshButton.vue";
import type { AcmeCertificateController } from "./acme-certificate-contract";

const props = defineProps<{ controller: AcmeCertificateController }>();
const {
  acmeStatusBadgeVariant,
  acmeStatusLabel,
  configStore,
  dnsProviders,
  goToAcmeInitialization,
  isAcmeInstalled,
  isDialogSubmitting,
  isOverviewLoading,
  isProvidersLoading,
  isTableLocked,
  lockReasonLabel,
  openCreateDialog,
  refresh,
  shouldPromptAcmeInitialization,
  t,
} = props.controller;
</script>

<template>
<Card class="border-border/80 shadow-sm">
      <CardHeader>
        <div
          class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
        >
          <div class="grid gap-1">
            <CardTitle class="flex flex-wrap items-center gap-2">
              {{
                configStore.isWindowsDeployment
                  ? t("admin.acmeCert.dns01Title")
                  : t("admin.acmeCert.title")
              }}
              <Badge :variant="acmeStatusBadgeVariant">{{
                acmeStatusLabel
              }}</Badge>
              <Badge v-if="isTableLocked" variant="outline">
                {{ lockReasonLabel }}
              </Badge>
            </CardTitle>
            <CardDescription>
              {{
                configStore.isWindowsDeployment
                  ? t("admin.acmeCert.dns01Description")
                  : t("admin.acmeCert.description")
              }}
            </CardDescription>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <RefreshButton
              :loading="isOverviewLoading || isProvidersLoading"
              :disabled="isOverviewLoading || isProvidersLoading"
              @click="refresh"
            />
            <Button
              :disabled="
                !isAcmeInstalled ||
                isDialogSubmitting ||
                !dnsProviders.length
              "
              @click="openCreateDialog"
            >
              {{ t("admin.acmeCert.newApplication") }}
            </Button>
          </div>
        </div>
      </CardHeader>
    </Card>

    <Alert
      v-if="shouldPromptAcmeInitialization"
      class="border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900/50 dark:bg-amber-950/20 dark:text-amber-100"
    >
      <AlertTriangle class="h-4 w-4" />
      <AlertTitle>
        {{ t("admin.acmeCert.initializePromptTitle") }}
      </AlertTitle>
      <AlertDescription
        class="grid gap-3 text-amber-900 sm:grid-cols-[1fr_auto] sm:items-center dark:text-amber-100/90"
      >
        <span>{{ t("admin.acmeCert.initializePromptDescription") }}</span>
        <Button
          type="button"
          size="sm"
          variant="outline"
          class="shrink-0 border-amber-300 bg-background/80 text-amber-950 hover:bg-background dark:border-amber-700 dark:text-amber-100"
          @click="goToAcmeInitialization"
        >
          {{ t("admin.acmeCert.goInitialize") }}
        </Button>
      </AlertDescription>
    </Alert>
</template>
