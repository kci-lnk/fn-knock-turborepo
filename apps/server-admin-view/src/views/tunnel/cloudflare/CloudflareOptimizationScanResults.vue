<script setup lang="ts">
import { computed } from "vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { LoaderCircle, ShieldCheck, TriangleAlert } from "lucide-vue-next";
import {
  formatOptimizationNumber,
  optimizationCandidateSourceLabel,
  optimizationScanErrorPresentation,
  optimizationScanPhaseLabel,
  optimizationSourceWarningLabel,
  optimizationVantageLabel,
} from "./cloudflareOptimizationPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  applyOptimization,
  isApplyingOptimization,
  optimizationApplied,
  optimizationScan,
  selectedCandidateIp,
  t,
} = controller;

const selectedCandidate = computed(() =>
  optimizationScan.value?.candidates.find(
    (candidate) => candidate.ip === selectedCandidateIp.value,
  ),
);
const formatNumber = formatOptimizationNumber;
const scanPhaseLabel = (phase: string) => optimizationScanPhaseLabel(phase, t);
const candidateSourceLabel = (candidate: {
  sourceHostnames: string[];
  sourceTypes: string[];
}) => optimizationCandidateSourceLabel(candidate, t);
const sourceWarningLabel = (warning: string) =>
  optimizationSourceWarningLabel(warning, t);
const vantageLabel = (
  vantage: Parameters<typeof optimizationVantageLabel>[0],
) => optimizationVantageLabel(vantage, t);
const scanError = computed(() =>
  optimizationScanErrorPresentation(
    optimizationScan.value?.errorCode,
    optimizationScan.value?.error,
    t,
  ),
);
</script>

<template>
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
  <Alert
    v-if="
      optimizationScan.status === 'completed' &&
      optimizationScan.preferredIp &&
      optimizationScan.preferredIpValidated === true
    "
    class="items-start"
  >
    <ShieldCheck class="size-4" />
    <AlertDescription>
      {{
        t("admin.cloudflareTunnel.optimization.preferredIpValidated", {
          ip: optimizationScan.preferredIp,
        })
      }}
    </AlertDescription>
  </Alert>
  <Alert
    v-else-if="
      optimizationScan.status === 'completed' &&
      optimizationScan.preferredIp &&
      optimizationScan.preferredIpValidated === false
    "
    class="items-start"
  >
    <TriangleAlert class="size-4" />
    <AlertDescription>
      {{
        t("admin.cloudflareTunnel.optimization.preferredIpRejected", {
          ip: optimizationScan.preferredIp,
        })
      }}
    </AlertDescription>
  </Alert>
  <div
    v-if="optimizationScan.vantage"
    class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground"
  >
    <span>
      {{ t("admin.cloudflareTunnel.optimization.vantage") }}:
      {{ vantageLabel(optimizationScan.vantage) }}
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
        {{ sourceWarningLabel(warning) }}
      </div>
    </AlertDescription>
  </Alert>
  <Alert
    v-if="optimizationScan.error"
    :variant="scanError.neutral ? 'default' : 'destructive'"
    class="items-start"
  >
    <TriangleAlert class="size-4" />
    <AlertTitle v-if="scanError.title">{{
      scanError.title
    }}</AlertTitle>
    <AlertDescription>{{ scanError.message }}</AlertDescription>
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
            <TableHead>{{
              t("admin.cloudflareTunnel.optimization.ipv4")
            }}</TableHead>
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
</template>
