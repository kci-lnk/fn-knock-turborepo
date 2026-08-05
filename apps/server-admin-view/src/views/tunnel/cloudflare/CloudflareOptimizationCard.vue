<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
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
  fallbackOptimization,
  isApplyingOptimization,
  isFallingBackOptimization,
  isScanningOptimization,
  optimization,
  optimizationApplied,
  optimizationEnabled,
  optimizationScan,
  selectedCandidateIp,
  startOptimizationScan,
  t,
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
const scanPhaseLabel = (phase: string) => {
  const key = phaseKeys[phase];
  return key
    ? t(`admin.cloudflareTunnel.optimization.phases.${key}`)
    : phase;
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
</script>

<template>
  <Card
    v-if="
      apiTokenConfigured &&
      (optimizationEnabled ||
        optimizationApplied ||
        optimization?.capabilityProbe?.status === 'unsupported')
    "
  >
    <CardHeader>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <CardTitle class="flex items-center gap-2">
          <Zap class="size-5" />
          {{ t("admin.cloudflareTunnel.optimization.title") }}
          <Badge variant="secondary">IPv4 Beta</Badge>
        </CardTitle>
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
    </CardHeader>
    <CardContent class="space-y-5">
      <Alert v-if="!optimizationApplied" class="items-start">
        <TriangleAlert class="size-4" />
        <AlertTitle>
          {{
            t(
              "admin.cloudflareTunnel.optimization.reconcileRequiredTitle",
            )
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

      <Alert class="items-start">
        <TriangleAlert class="size-4" />
        <AlertTitle>{{
          t("admin.cloudflareTunnel.optimization.betaTitle")
        }}</AlertTitle>
        <AlertDescription>
          {{ t("admin.cloudflareTunnel.optimization.betaDescription") }}
        </AlertDescription>
      </Alert>

      <Alert
        v-if="optimization?.capabilityProbe"
        :variant="
          optimization.capabilityProbe.status === 'unsupported'
            ? 'destructive'
            : 'default'
        "
        class="items-start"
      >
        <ShieldCheck class="size-4" />
        <AlertTitle>
          {{ t("admin.cloudflareTunnel.optimization.capabilityProbe") }}
        </AlertTitle>
        <AlertDescription>
          {{
            optimization.capabilityProbe.message ||
            t(
              `admin.cloudflareTunnel.optimization.capability.${optimization.capabilityProbe.status}`,
            )
          }}
        </AlertDescription>
      </Alert>

      <div class="grid gap-3 sm:grid-cols-4">
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
            {{ t("admin.cloudflareTunnel.optimization.lastHealth") }}
          </div>
          <div class="mt-1 text-sm">
            {{ formatDate(optimization?.schedule.lastHealthAt) }}
          </div>
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
            {{
              optimization?.domains.filter((item) => item.optimized).length || 0
            }}
            / {{ optimization?.domains.length || 0 }}
          </div>
        </div>
      </div>

      <div
        v-if="optimization?.schedule.lastSwitchReason"
        class="text-sm text-muted-foreground"
      >
        {{ t("admin.cloudflareTunnel.optimization.lastSwitchReason") }}:
        {{ switchReasonLabel(optimization.schedule.lastSwitchReason) }}
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
        <Alert
          v-if="optimizationScan.error"
          variant="destructive"
          class="items-start"
        >
          <TriangleAlert class="size-4" />
          <AlertDescription>{{ optimizationScan.error }}</AlertDescription>
        </Alert>

        <div v-if="optimizationScan.candidates.length" class="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>IPv4</TableHead>
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
                <TableCell
                  >{{ formatNumber(candidate.medianLatencyMs) }} ms</TableCell
                >
                <TableCell
                  >{{ formatNumber(candidate.lossRatio * 100) }}%</TableCell
                >
                <TableCell
                  >{{ formatNumber(candidate.downloadMbps) }} Mbps</TableCell
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
                        ? t("admin.cloudflareTunnel.optimization.recommended")
                        : t("admin.cloudflareTunnel.optimization.select")
                    }}
                  </Button>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <div class="mt-3 flex justify-end">
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
        </div>
      </div>

      <div v-if="optimization?.domains.length" class="space-y-2">
        <div class="text-sm font-medium">
          {{ t("admin.cloudflareTunnel.optimization.domainStatus") }}
        </div>
        <div class="grid gap-2 sm:grid-cols-2">
          <div
            v-for="domain in optimization.domains"
            :key="domain.hostname"
            class="flex items-start justify-between gap-3 rounded-md border px-3 py-2"
          >
            <div class="min-w-0">
              <div class="truncate font-mono text-sm">
                {{ domain.hostname }}
              </div>
              <div v-if="domain.message" class="mt-1 text-xs text-destructive">
                {{ domain.message }}
              </div>
            </div>
            <Badge :variant="domain.optimized ? 'default' : 'secondary'">
              {{ domainStatusLabel(domain.status) }}
            </Badge>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
