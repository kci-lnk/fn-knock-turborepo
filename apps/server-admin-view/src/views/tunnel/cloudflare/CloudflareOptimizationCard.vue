<script setup lang="ts">
import { computed } from "vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Textarea } from "@/components/ui/textarea";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  Zap,
} from "lucide-vue-next";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  apiTokenConfigured,
  applyOptimization,
  cancelOptimizationScan,
  configLoaded,
  fallbackOptimization,
  isApplyingOptimization,
  isFallingBackOptimization,
  isLoadingManagedState,
  isSavingOptimizationSources,
  isScanningOptimization,
  optimization,
  optimizationApplied,
  optimizationBuiltinIds,
  optimizationCustomHostnames,
  optimizationEnabled,
  optimizationOfficialRanges,
  optimizationScan,
  saveOptimizationSources,
  selectedCandidateIp,
  startOptimizationScan,
  t,
  toggleOptimizationBuiltin,
} = controller;

const formatNumber = (value: number, digits = 1) =>
  Number.isFinite(value) ? value.toFixed(digits) : "-";

const formatDate = (value?: string | null) => {
  if (!value) return "-";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
};

const phaseKeys: Record<string, string> = {
  queued: "queued",
  latency: "latency",
  download: "download",
  completed: "completed",
  failed: "failed",
  cancelled: "cancelled",
};
const domainStatusKeys: Record<string, string> = {
  active: "active",
  pending: "pending",
  ready: "ready",
  optimized: "optimized",
  fallback: "fallback",
  conflict: "conflict",
  quota: "quota",
  queued: "queued",
  "probe-failed": "probeFailed",
};
const switchReasonKeys: Record<string, string> = {
  "manual-speed-test": "manualSpeedTest",
  "manual-fallback": "manualFallback",
  "health-failover": "healthFailover",
  "health-fallback": "healthFallback",
};
const cloudflareSaasRequiredErrorCode = "cloudflare-saas-required";
const legacyCloudflareSaasErrorMarkers = [
  "no active business or capability hostname",
  "not entitled",
  "not enabled for this zone",
  "not available on your plan",
  "plan does not support",
  "requires an enterprise plan",
  "upgrade your plan",
  "no quota has been allocated",
  "(1404)",
];
const requiresCloudflareSaasSetup = (
  errorCode?: string | null,
  message?: string | null,
) => {
  if (errorCode === cloudflareSaasRequiredErrorCode) return true;
  const normalized = message?.toLowerCase();
  return (
    normalized !== undefined &&
    legacyCloudflareSaasErrorMarkers.some((marker) =>
      normalized.includes(marker),
    )
  );
};
const scanPhaseLabel = (phase: string) => {
  const key = phaseKeys[phase];
  return key ? t(`admin.cloudflareTunnel.optimization.phases.${key}`) : phase;
};
const domainStatusLabel = (status: string) => {
  const key = domainStatusKeys[status];
  return key
    ? t(`admin.cloudflareTunnel.optimization.domainStatuses.${key}`)
    : status;
};
const switchReasonLabel = (reason: string) => {
  const key = switchReasonKeys[reason];
  return key
    ? t(`admin.cloudflareTunnel.optimization.switchReasons.${key}`)
    : reason;
};
const builtinLabel = (id: string, hostname: string) => {
  const key = `admin.cloudflareTunnel.optimization.sources.builtins.${id}`;
  const translated = t(key);
  return translated === key ? hostname : translated;
};
const candidateSourceLabel = (candidate: {
  sourceHostnames: string[];
  sourceTypes: string[];
}) => {
  if (candidate.sourceHostnames.length) {
    return candidate.sourceHostnames.join(", ");
  }
  return candidate.sourceTypes.includes("official-range")
    ? t("admin.cloudflareTunnel.optimization.sources.officialRangesShort")
    : "-";
};
const optimizedDomainCount = computed(
  () =>
    optimization.value?.domains.filter((item) => item.optimized).length || 0,
);
const selectedCandidate = computed(() =>
  optimizationScan.value?.candidates.find(
    (candidate) => candidate.ip === selectedCandidateIp.value,
  ),
);
const capabilityRequiresCloudflareSaas = computed(() => {
  const probe = optimization.value?.capabilityProbe;
  return requiresCloudflareSaasSetup(probe?.reasonCode, probe?.message);
});
const capabilityProbeMessage = computed(() => {
  const probe = optimization.value?.capabilityProbe;
  if (!probe) return "";
  if (capabilityRequiresCloudflareSaas.value) {
    return t(
      "admin.cloudflareTunnel.optimization.cloudflareSaasRequiredDescription",
    );
  }
  return (
    probe.message ||
    t(`admin.cloudflareTunnel.optimization.capability.${probe.status}`)
  );
});
const scanRequiresCloudflareSaas = computed(() =>
  requiresCloudflareSaasSetup(
    optimizationScan.value?.errorCode,
    optimizationScan.value?.error,
  ),
);
const scanErrorMessage = computed(() =>
  scanRequiresCloudflareSaas.value
    ? t(
        "admin.cloudflareTunnel.optimization.cloudflareSaasRequiredDescription",
      )
    : optimizationScan.value?.error || "",
);
</script>

<template>
  <ConfigCollapsibleCard
    v-if="
      apiTokenConfigured &&
      (optimizationEnabled ||
        optimizationApplied ||
        optimization?.capabilityProbe?.status === 'unsupported')
    "
    :title="t('admin.cloudflareTunnel.optimization.title')"
    :configured="optimizationApplied"
    :ready="configLoaded && !isLoadingManagedState"
    :edit-label="t('admin.cloudflareTunnel.managed.viewOrChange')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        optimizationApplied
          ? optimization?.fallbackActive
            ? t("admin.cloudflareTunnel.optimization.summaryFallback")
            : t("admin.cloudflareTunnel.optimization.summaryActive", {
                count: optimizedDomainCount,
                total: optimization?.domains.length || 0,
                ip: optimization?.selected?.ip || "-",
              })
          : t("admin.cloudflareTunnel.optimization.summaryNotApplied")
      }}
    </template>

    <template #default>
      <div class="space-y-5 p-4 sm:p-6">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="max-w-3xl">
            <div class="flex items-center gap-2 text-base font-semibold">
              <Zap class="size-5" />
              {{ t("admin.cloudflareTunnel.optimization.heading") }}
              <Badge variant="secondary">Beta</Badge>
            </div>
            <p class="mt-1 text-sm text-muted-foreground">
              {{ t("admin.cloudflareTunnel.optimization.description") }}
            </p>
          </div>
          <Badge
            :variant="optimization?.fallbackActive ? 'secondary' : 'default'"
          >
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
            {{
              t("admin.cloudflareTunnel.optimization.reconcileRequiredTitle")
            }}
          </AlertTitle>
          <AlertDescription>
            {{
              t(
                "admin.cloudflareTunnel.optimization.reconcileRequiredDescription",
              )
            }}
          </AlertDescription>
        </Alert>

        <details class="rounded-lg border bg-muted/20">
          <summary class="cursor-pointer list-none px-4 py-3">
            <div class="text-sm font-medium">
              {{
                t("admin.cloudflareTunnel.optimization.sources.advancedTitle")
              }}
            </div>
            <div class="mt-1 text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.optimization.sources.description") }}
            </div>
          </summary>
          <div class="space-y-4 border-t p-4">
            <Alert
              v-if="optimization?.candidateSources.error"
              variant="destructive"
              class="items-start"
            >
              <TriangleAlert class="size-4" />
              <AlertDescription>
                {{ optimization.candidateSources.error }}
              </AlertDescription>
            </Alert>

            <div class="flex items-start gap-3 rounded-md border p-3">
              <Checkbox
                id="optimization-official-ranges"
                v-model="optimizationOfficialRanges"
              />
              <Label
                for="optimization-official-ranges"
                class="grid cursor-pointer gap-1 font-normal"
              >
                <span class="text-sm font-medium">
                  {{
                    t(
                      "admin.cloudflareTunnel.optimization.sources.officialRanges",
                    )
                  }}
                </span>
                <span class="text-xs text-muted-foreground">
                  {{
                    t(
                      "admin.cloudflareTunnel.optimization.sources.officialRangesDescription",
                    )
                  }}
                </span>
              </Label>
            </div>

            <div>
              <div class="mb-2 text-sm font-medium">
                {{
                  t("admin.cloudflareTunnel.optimization.sources.builtinTitle")
                }}
              </div>
              <div class="grid gap-2 sm:grid-cols-2">
                <div
                  v-for="source in optimization?.candidateSources.builtins ||
                  []"
                  :key="source.id"
                  class="flex items-start gap-3 rounded-md border p-3"
                >
                  <Checkbox
                    :id="`optimization-source-${source.id}`"
                    :model-value="optimizationBuiltinIds.includes(source.id)"
                    @update:model-value="
                      (value) =>
                        toggleOptimizationBuiltin(source.id, value === true)
                    "
                  />
                  <Label
                    :for="`optimization-source-${source.id}`"
                    class="grid min-w-0 cursor-pointer gap-1 font-normal"
                  >
                    <span class="text-sm font-medium">
                      {{ builtinLabel(source.id, source.hostname) }}
                    </span>
                    <code class="truncate text-xs text-muted-foreground">{{
                      source.hostname
                    }}</code>
                  </Label>
                </div>
              </div>
            </div>

            <div class="space-y-2">
              <Label for="optimization-custom-hostnames">
                {{
                  t("admin.cloudflareTunnel.optimization.sources.customTitle")
                }}
              </Label>
              <Textarea
                id="optimization-custom-hostnames"
                v-model="optimizationCustomHostnames"
                :rows="4"
                :placeholder="
                  t(
                    'admin.cloudflareTunnel.optimization.sources.customPlaceholder',
                  )
                "
              />
              <div class="text-xs text-muted-foreground">
                {{
                  t(
                    "admin.cloudflareTunnel.optimization.sources.customDescription",
                    {
                      max:
                        optimization?.candidateSources.maxCustomHostnames || 16,
                    },
                  )
                }}
              </div>
            </div>

            <Alert class="items-start">
              <ShieldCheck class="size-4" />
              <AlertDescription>
                {{ t("admin.cloudflareTunnel.optimization.sources.safety") }}
              </AlertDescription>
            </Alert>

            <div class="flex justify-end">
              <Button
                variant="outline"
                :disabled="isSavingOptimizationSources"
                @click="saveOptimizationSources"
              >
                <LoaderCircle
                  v-if="isSavingOptimizationSources"
                  class="mr-2 size-4 animate-spin"
                />
                {{ t("admin.cloudflareTunnel.optimization.sources.save") }}
              </Button>
            </div>
          </div>
        </details>

        <Alert
          v-if="
            optimization?.capabilityProbe &&
            optimization.capabilityProbe.status === 'unsupported'
          "
          variant="destructive"
          class="items-start"
        >
          <TriangleAlert
            v-if="capabilityRequiresCloudflareSaas"
            class="size-4"
          />
          <ShieldCheck v-else class="size-4" />
          <AlertTitle>
            {{
              capabilityRequiresCloudflareSaas
                ? t(
                    "admin.cloudflareTunnel.optimization.cloudflareSaasRequiredTitle",
                  )
                : t("admin.cloudflareTunnel.optimization.capabilityProbe")
            }}
          </AlertTitle>
          <AlertDescription>
            {{ capabilityProbeMessage }}
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
              {{ formatDate(optimization?.schedule.nextFullScanAt) }}
            </div>
          </div>
          <div class="rounded-md border p-3">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.optimization.optimizedDomains") }}
            </div>
            <div class="mt-1 text-sm font-medium">
              {{ optimizedDomainCount }}
              / {{ optimization?.domains.length || 0 }}
            </div>
          </div>
        </div>

        <div class="flex flex-wrap gap-2">
          <Button
            :disabled="!optimizationApplied || isScanningOptimization"
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

        <div v-if="optimizationScan" class="space-y-3 rounded-lg border p-4">
          <div class="flex items-center justify-between text-sm">
            <span>
              {{
                t("admin.cloudflareTunnel.optimization.scanPhase", {
                  phase: scanPhaseLabel(optimizationScan.phase),
                })
              }}
            </span>
            <span>{{ optimizationScan.progress }}%</span>
          </div>
          <Progress :model-value="optimizationScan.progress" />
          <div
            v-if="optimizationScan.vantage"
            class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground"
          >
            <span>
              {{ t("admin.cloudflareTunnel.optimization.vantage") }}:
              {{ optimizationScan.vantage.label }}
            </span>
            <span>
              {{ t("admin.cloudflareTunnel.optimization.publicIp") }}:
              <code>{{ optimizationScan.vantage.publicIp || "-" }}</code>
            </span>
            <span>
              {{ t("admin.cloudflareTunnel.optimization.defaultColo") }}:
              <code>{{ optimizationScan.vantage.defaultColo || "-" }}</code>
            </span>
          </div>
          <Alert
            v-if="optimizationScan.sourceWarnings.length"
            class="items-start"
          >
            <TriangleAlert class="size-4" />
            <AlertDescription>
              <div
                v-for="warning in optimizationScan.sourceWarnings"
                :key="warning"
              >
                {{ warning }}
              </div>
            </AlertDescription>
          </Alert>
          <Alert
            v-if="optimizationScan.error"
            variant="destructive"
            class="items-start"
          >
            <TriangleAlert class="size-4" />
            <AlertTitle v-if="scanRequiresCloudflareSaas">
              {{
                t(
                  "admin.cloudflareTunnel.optimization.cloudflareSaasRequiredTitle",
                )
              }}
            </AlertTitle>
            <AlertDescription>{{ scanErrorMessage }}</AlertDescription>
          </Alert>

          <div
            v-if="selectedCandidate"
            class="flex flex-wrap items-center justify-between gap-4 rounded-lg border bg-primary/5 p-4"
          >
            <div>
              <div class="text-xs text-muted-foreground">
                {{ t("admin.cloudflareTunnel.optimization.selectedResult") }}
              </div>
              <div class="mt-1 flex flex-wrap items-center gap-3">
                <code class="font-medium">{{ selectedCandidate.ip }}</code>
                <Badge variant="outline">
                  {{
                    selectedCandidate.businessColo ||
                    selectedCandidate.colo ||
                    "-"
                  }}
                </Badge>
                <span class="text-xs text-muted-foreground">
                  {{ formatNumber(selectedCandidate.medianLatencyMs) }} ms ·
                  {{ formatNumber(selectedCandidate.downloadMbps) }} Mbps
                </span>
              </div>
            </div>
            <Button
              :disabled="
                !optimizationApplied ||
                !selectedCandidateIp ||
                isApplyingOptimization
              "
              @click="applyOptimization"
            >
              <LoaderCircle
                v-if="isApplyingOptimization"
                class="mr-2 size-4 animate-spin"
              />
              {{ t("admin.cloudflareTunnel.optimization.apply") }}
            </Button>
          </div>

          <details
            v-if="optimizationScan.candidates.length"
            class="rounded-lg border bg-muted/20"
          >
            <summary
              class="cursor-pointer list-none px-4 py-3 text-sm font-medium"
            >
              {{
                t("admin.cloudflareTunnel.optimization.allCandidates", {
                  count: optimizationScan.candidates.length,
                })
              }}
            </summary>
            <div class="overflow-x-auto border-t">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>IPv4</TableHead>
                    <TableHead>{{
                      t("admin.cloudflareTunnel.optimization.source")
                    }}</TableHead>
                    <TableHead>{{
                      t("admin.cloudflareTunnel.optimization.colo")
                    }}</TableHead>
                    <TableHead>{{
                      t("admin.cloudflareTunnel.optimization.latency")
                    }}</TableHead>
                    <TableHead>{{
                      t("admin.cloudflareTunnel.optimization.loss")
                    }}</TableHead>
                    <TableHead>{{
                      t("admin.cloudflareTunnel.optimization.bandwidth")
                    }}</TableHead>
                    <TableHead>{{
                      t("admin.cloudflareTunnel.optimization.score")
                    }}</TableHead>
                    <TableHead class="text-right">
                      {{ t("admin.cloudflareTunnel.optimization.selection") }}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow
                    v-for="candidate in optimizationScan.candidates"
                    :key="candidate.ip"
                    :class="{
                      'bg-primary/5': selectedCandidateIp === candidate.ip,
                    }"
                  >
                    <TableCell class="font-mono">{{ candidate.ip }}</TableCell>
                    <TableCell class="max-w-48 truncate text-xs">
                      {{ candidateSourceLabel(candidate) }}
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{{
                        candidate.businessColo || candidate.colo || "-"
                      }}</Badge>
                    </TableCell>
                    <TableCell
                      >{{
                        formatNumber(candidate.medianLatencyMs)
                      }}
                      ms</TableCell
                    >
                    <TableCell
                      >{{ formatNumber(candidate.lossRatio * 100) }}%</TableCell
                    >
                    <TableCell
                      >{{
                        formatNumber(candidate.downloadMbps)
                      }}
                      Mbps</TableCell
                    >
                    <TableCell>{{ formatNumber(candidate.score) }}</TableCell>
                    <TableCell class="text-right">
                      <Button
                        size="sm"
                        :variant="
                          selectedCandidateIp === candidate.ip
                            ? 'default'
                            : 'outline'
                        "
                        @click="selectedCandidateIp = candidate.ip"
                      >
                        {{
                          candidate.ip === optimizationScan.recommendedIp
                            ? t(
                                "admin.cloudflareTunnel.optimization.recommended",
                              )
                            : t("admin.cloudflareTunnel.optimization.select")
                        }}
                      </Button>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>
          </details>
        </div>

        <details
          v-if="optimization?.domains.length"
          class="rounded-lg border bg-muted/20"
        >
          <summary
            class="cursor-pointer list-none px-4 py-3 text-sm font-medium"
          >
            {{ t("admin.cloudflareTunnel.optimization.domainStatus") }}
          </summary>
          <div class="grid gap-2 border-t p-4 sm:grid-cols-2">
            <div
              v-for="domain in optimization.domains"
              :key="domain.hostname"
              class="flex items-start justify-between gap-3 rounded-md border px-3 py-2"
            >
              <div class="min-w-0">
                <div class="truncate font-mono text-sm">
                  {{ domain.hostname }}
                </div>
                <div
                  v-if="domain.message"
                  class="mt-1 text-xs text-destructive"
                >
                  {{ domain.message }}
                </div>
              </div>
              <Badge :variant="domain.optimized ? 'default' : 'secondary'">
                {{ domainStatusLabel(domain.status) }}
              </Badge>
            </div>
          </div>
        </details>

        <details class="rounded-lg border bg-muted/20">
          <summary
            class="cursor-pointer list-none px-4 py-3 text-sm font-medium"
          >
            {{ t("admin.cloudflareTunnel.optimization.technicalStatus") }}
          </summary>
          <div class="grid gap-3 border-t p-4 text-sm sm:grid-cols-2">
            <div>
              <div class="text-xs text-muted-foreground">
                {{ t("admin.cloudflareTunnel.optimization.capabilityProbe") }}
              </div>
              <div class="mt-1">
                {{ capabilityProbeMessage }}
              </div>
            </div>
            <div>
              <div class="text-xs text-muted-foreground">
                {{ t("admin.cloudflareTunnel.optimization.lastHealth") }}
              </div>
              <div class="mt-1">
                {{ formatDate(optimization?.schedule.lastHealthAt) }}
              </div>
            </div>
            <div v-if="optimization?.schedule.lastSwitchReason">
              <div class="text-xs text-muted-foreground">
                {{ t("admin.cloudflareTunnel.optimization.lastSwitchReason") }}
              </div>
              <div class="mt-1">
                {{ switchReasonLabel(optimization.schedule.lastSwitchReason) }}
              </div>
            </div>
          </div>
        </details>
      </div>
    </template>

    <template #actions="{ collapse }">
      <div
        class="flex justify-end rounded-b-lg border-t bg-muted/30 p-4 sm:px-6"
      >
        <Button variant="outline" @click="collapse">
          {{ t("admin.cloudflareTunnel.collapse") }}
        </Button>
      </div>
    </template>
  </ConfigCollapsibleCard>
</template>
