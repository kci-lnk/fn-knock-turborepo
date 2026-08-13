<script setup lang="ts">
import {
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  Zap,
} from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { CloudflareOptimizationCardPresentation } from "./useCloudflareOptimizationCardPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const props = defineProps<{
  controller: CloudflareTunnelController;
  presentation: CloudflareOptimizationCardPresentation;
}>();
const {
  cancelOptimizationScan,
  fallbackOptimization,
  isFallingBackOptimization,
  isScanningOptimization,
  optimization,
  optimizationApplied,
  optimizationScanReady,
  preferredCandidateIp,
  startOptimizationScan,
  t,
} = props.controller;
</script>

<template>
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div class="max-w-3xl">
      <div class="flex items-center gap-2 text-base font-semibold">
        <Zap class="size-5" />
        {{ t("admin.cloudflareTunnel.optimization.heading") }}
        <Badge variant="secondary">
          {{ t("admin.cloudflareTunnel.optimization.betaBadge") }}
        </Badge>
      </div>
      <p class="mt-1 text-sm text-muted-foreground">
        {{ t("admin.cloudflareTunnel.optimization.description") }}
      </p>
    </div>
    <Badge :variant="optimization?.fallbackActive ? 'secondary' : 'default'">
      {{
        optimization?.fallbackActive
          ? t("admin.cloudflareTunnel.optimization.fallbackStatus")
          : t("admin.cloudflareTunnel.optimization.activeStatus")
      }}
    </Badge>
  </div>

  <Alert v-if="!optimizationApplied" class="items-start">
    <TriangleAlert class="size-4" />
    <AlertTitle>
      {{ t("admin.cloudflareTunnel.optimization.reconcileRequiredTitle") }}
    </AlertTitle>
    <AlertDescription>
      {{
        t("admin.cloudflareTunnel.optimization.reconcileRequiredDescription")
      }}
    </AlertDescription>
  </Alert>

  <slot name="source-settings" />

  <Alert
    v-if="optimization?.capabilityProbe?.status === 'unsupported'"
    variant="destructive"
    class="items-start"
  >
    <TriangleAlert
      v-if="presentation.capabilityRequiresCloudflareSaas"
      class="size-4"
    />
    <ShieldCheck v-else class="size-4" />
    <AlertTitle>
      {{
        presentation.capabilityRequiresCloudflareSaas
          ? t(
              "admin.cloudflareTunnel.optimization.cloudflareSaasRequiredTitle",
            )
          : t("admin.cloudflareTunnel.optimization.capabilityProbe")
      }}
    </AlertTitle>
    <AlertDescription>
      {{ presentation.capabilityProbeMessage }}
    </AlertDescription>
  </Alert>

  <div class="grid gap-3 sm:grid-cols-3">
    <div class="rounded-md border p-3">
      <div class="text-xs text-muted-foreground">
        {{ t("admin.cloudflareTunnel.optimization.currentIp") }}
      </div>
      <code class="mt-1 block text-sm">{{
        optimization?.selected?.ip || "-"
      }}</code>
    </div>
    <div class="rounded-md border p-3">
      <div class="text-xs text-muted-foreground">
        {{ t("admin.cloudflareTunnel.optimization.nextScan") }}
      </div>
      <div class="mt-1 text-sm">
        {{ presentation.formatDate(optimization?.schedule.nextFullScanAt) }}
      </div>
    </div>
    <div class="rounded-md border p-3">
      <div class="text-xs text-muted-foreground">
        {{ t("admin.cloudflareTunnel.optimization.optimizedDomains") }}
      </div>
      <div class="mt-1 text-sm font-medium">
        {{ presentation.optimizedDomainCount }} /
        {{ presentation.optimizationManagedDomainCount }}
      </div>
    </div>
  </div>

  <div class="grid gap-2 rounded-md border bg-muted/20 p-3 sm:max-w-xl">
    <Label for="optimization-preferred-ip">
      {{ t("admin.cloudflareTunnel.optimization.preferredIpLabel") }}
    </Label>
    <Input
      id="optimization-preferred-ip"
      v-model="preferredCandidateIp"
      aria-describedby="optimization-preferred-ip-description"
      inputmode="decimal"
      autocomplete="off"
      :disabled="isScanningOptimization"
      :placeholder="
        t('admin.cloudflareTunnel.optimization.preferredIpPlaceholder')
      "
    />
    <div
      id="optimization-preferred-ip-description"
      class="text-xs text-muted-foreground"
    >
      {{ t("admin.cloudflareTunnel.optimization.preferredIpDescription") }}
    </div>
  </div>

  <div class="flex flex-wrap gap-2">
    <Button
      :disabled="
        !optimizationApplied ||
        !optimizationScanReady ||
        isScanningOptimization
      "
      @click="startOptimizationScan"
    >
      <RefreshCw
        class="mr-2 size-4"
        :class="{ 'animate-spin': isScanningOptimization }"
      />
      {{ t("admin.cloudflareTunnel.optimization.startScan") }}
    </Button>
    <Button
      v-if="isScanningOptimization"
      variant="outline"
      @click="cancelOptimizationScan"
    >
      {{ t("admin.cloudflareTunnel.optimization.cancelScan") }}
    </Button>
    <Button
      variant="outline"
      :disabled="
        !optimizationApplied ||
        isFallingBackOptimization ||
        optimization?.fallbackActive
      "
      @click="fallbackOptimization"
    >
      {{ t("admin.cloudflareTunnel.optimization.fallback") }}
    </Button>
  </div>

  <Alert
    v-if="
      optimizationApplied &&
      !optimizationScanReady &&
      !presentation.capabilityRequiresCloudflareSaas
    "
    class="items-start"
  >
    <LoaderCircle
      v-if="presentation.capabilityValidationPending"
      class="size-4 animate-spin"
    />
    <TriangleAlert v-else class="size-4" />
    <AlertTitle>
      {{
        presentation.optimizationResourceConflict
          ? t("admin.cloudflareTunnel.optimization.resourceConflictTitle")
          : presentation.capabilityValidationPending
            ? t(
                "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingTitle",
              )
            : t("admin.cloudflareTunnel.optimization.notReadyTitle")
      }}
    </AlertTitle>
    <AlertDescription>
      {{
        presentation.optimizationResourceConflict
          ? t(
              "admin.cloudflareTunnel.optimization.resourceConflictDescription",
            )
          : presentation.capabilityValidationPending
            ? t(
                "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingDescription",
              )
            : t("admin.cloudflareTunnel.optimization.notReadyDescription")
      }}
    </AlertDescription>
  </Alert>
</template>
