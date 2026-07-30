<template>
  <Card class="border-border/80 shadow-sm">
    <CardHeader>
      <div class="flex items-start justify-between gap-4">
        <div class="grid gap-1">
          <CardTitle class="flex items-center gap-2">
            {{ t("admin.acmeJobPanel.title") }}
            <Badge :variant="jobBadgeVariant">{{ jobStatusLabel }}</Badge>
          </CardTitle>
          <CardDescription class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span v-if="applicationLabel" class="text-sm">{{
              applicationLabel
            }}</span>
            <span
              v-if="applicationLabel && props.job.domains.length"
              class="text-xs text-muted-foreground"
            >
              ·
            </span>
            <span class="font-mono text-xs">{{
              props.job.domains.join(", ")
            }}</span>
            <span class="text-xs text-muted-foreground">·</span>
            <span class="font-mono text-xs text-muted-foreground">
              {{ props.job.provider || "-" }}
            </span>
          </CardDescription>
        </div>
        <div class="flex flex-wrap items-center justify-end gap-2">
          <ConfirmDangerPopover
            v-if="props.canStop"
            :title="t('admin.acmeJobPanel.confirmStopTitle')"
            :description="t('admin.acmeJobPanel.confirmStopDescription')"
            :confirm-text="t('admin.acmeJobPanel.stopTask')"
            :loading="props.isStopping"
            :disabled="props.isStopping"
            :on-confirm="props.stopAction || (() => undefined)"
            content-class="w-80 text-left"
          >
            <template #trigger>
              <Button
                type="button"
                variant="destructive"
                size="sm"
                :disabled="props.isStopping"
              >
                {{ t("admin.acmeJobPanel.stopTask") }}
              </Button>
            </template>
          </ConfirmDangerPopover>
          <Button
            type="button"
            variant="outline"
            size="sm"
            :disabled="props.isRefreshing"
            @click="emit('refresh')"
          >
            {{ t("admin.acmeJobPanel.refreshLogs") }}
          </Button>
        </div>
      </div>
    </CardHeader>

    <CardContent class="grid gap-4">
      <Alert
        v-if="props.analysis"
        :variant="analysisVariant"
        class="dynamic-white-glass-surface"
      >
        <component :is="analysisIcon" class="h-4 w-4" />
        <AlertTitle>{{ analysisTitle }}</AlertTitle>
        <AlertDescription>
          <div class="grid gap-2">
            <p>{{ props.analysis.message }}</p>
            <div class="flex flex-wrap items-center gap-2">
              <Button
                v-if="
                  props.analysis.reason === 'dns_credentials_invalid' ||
                  props.analysis.reason === 'dns_credentials_invalid_email'
                "
                type="button"
                variant="outline"
                size="sm"
                @click="emit('focus-credentials')"
              >
                {{ t("admin.acmeJobPanel.checkDnsCredentials") }}
              </Button>
              <Button
                v-if="props.analysis.evidence?.length"
                type="button"
                variant="outline"
                size="sm"
                @click="isAnalysisOpen = !isAnalysisOpen"
              >
                {{
                  isAnalysisOpen
                    ? t("admin.acmeJobPanel.collapse")
                    : t("admin.acmeJobPanel.view")
                }}
              </Button>
            </div>

            <Collapsible
              v-if="props.analysis.evidence?.length"
              v-model:open="isAnalysisOpen"
            >
              <CollapsibleContent>
                <div
                  class="w-full min-w-0 rounded-md border bg-muted/20 p-2 font-mono text-xs text-muted-foreground"
                >
                  <div
                    v-for="(line, idx) in props.analysis.evidence"
                    :key="idx"
                    class="whitespace-pre-wrap break-all"
                  >
                    {{ line }}
                  </div>
                </div>
              </CollapsibleContent>
            </Collapsible>
          </div>
        </AlertDescription>
      </Alert>

      <div class="grid gap-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-muted-foreground">
            {{ t("admin.acmeJobPanel.progress") }}
          </span>
          <span class="text-xs font-mono text-muted-foreground"
            >{{ jobProgress }}%</span
          >
        </div>
        <Progress :model-value="jobProgress" />
      </div>

      <LogViewer :logs="props.logs" />
    </CardContent>
  </Card>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { TriangleAlert, Info } from "lucide-vue-next";
import type { AcmeJobData, AcmeLogAnalysis } from "@/lib/api";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import { Progress } from "@/components/ui/progress";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import LogViewer from "@admin-shared/components/LogViewer.vue";

const { t } = useI18n();

const props = defineProps<{
  job: AcmeJobData;
  logs: string[];
  analysis?: AcmeLogAnalysis | null;
  applicationLabel?: string;
  isRefreshing?: boolean;
  canStop?: boolean;
  isStopping?: boolean;
  stopAction?: () => void | Promise<void>;
}>();

const emit = defineEmits<{
  refresh: [];
  "focus-credentials": [];
}>();

const isAnalysisOpen = ref(false);

watch(
  () => props.analysis,
  (nextAnalysis) => {
    if (!nextAnalysis) {
      isAnalysisOpen.value = false;
    }
  },
);

const jobProgress = computed(() => {
  const value = Number(props.job.progress ?? 0);
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, Math.round(value)));
});

const jobStatusLabel = computed(() => {
  if (props.job.status === "queued") return t("admin.acmeJobPanel.queued");
  if (props.job.status === "running") return t("admin.acmeJobPanel.running");
  if (props.job.status === "succeeded")
    return t("admin.acmeJobPanel.succeeded");
  if (props.job.status === "failed") return t("admin.acmeJobPanel.failed");
  if (props.job.status === "stopped") return t("admin.acmeJobPanel.stopped");
  return props.job.status || t("admin.acmeJobPanel.unknown");
});

const jobBadgeVariant = computed(() => {
  if (props.job.status === "succeeded") return "secondary";
  if (props.job.status === "failed") return "destructive";
  if (props.job.status === "stopped") return "outline";
  if (props.job.status === "running") return "default";
  return "outline";
});

const analysisVariant = computed(() => {
  if (!props.analysis) return "default";
  return props.analysis.reason === "unknown" ? "default" : "destructive";
});

const analysisIcon = computed(() => {
  if (!props.analysis) return Info;
  return analysisVariant.value === "destructive" ? TriangleAlert : Info;
});

const analysisTitle = computed(() => {
  const analysis = props.analysis;
  if (!analysis) return "";
  if (
    analysis.reason === "dns_credentials_invalid" ||
    analysis.reason === "dns_credentials_invalid_email"
  ) {
    return t("admin.acmeJobPanel.analysis.dnsCredentials");
  }
  if (analysis.reason === "dns_api_rate_limited") {
    return t("admin.acmeJobPanel.analysis.dnsRateLimited");
  }
  if (analysis.reason === "acme_frequency_limited") {
    return t("admin.acmeJobPanel.analysis.acmeFrequencyLimited");
  }
  return t("admin.acmeJobPanel.analysis.generic");
});
</script>
