<script setup lang="ts">
import { Activity, FileText, MemoryStick } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { RuntimeComponentHealth, RuntimeHealthStatus } from "../../types";

const props = withDefaults(
  defineProps<{
    component: RuntimeComponentHealth;
    variant: "process" | "service";
    showLogAction?: boolean;
    showMemoryAction?: boolean;
    showDebugAction?: boolean;
  }>(),
  { showLogAction: false, showMemoryAction: false, showDebugAction: false },
);

const emit = defineEmits<{
  viewLogs: [component: RuntimeComponentHealth];
  manageMemory: [component: RuntimeComponentHealth];
  viewDebug: [component: RuntimeComponentHealth];
}>();

const { t } = useI18n();

const formatDate = (value?: string | null) =>
  value ? new Date(value).toLocaleString() : "-";

const formatDuration = (milliseconds?: number | null) => {
  if (milliseconds === undefined || milliseconds === null) return "-";
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days) return `${days}d ${hours}h`;
  if (hours) return `${hours}h ${minutes}m`;
  if (minutes) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
};

const formatBytes = (bytes?: number | null) => {
  if (bytes === undefined || bytes === null) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
};

const statusClass = (status: RuntimeHealthStatus) => {
  if (status === "healthy")
    return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700";
  if (status === "degraded" || status === "blocked")
    return "border-amber-500/30 bg-amber-500/10 text-amber-700";
  if (status === "unhealthy")
    return "border-red-500/30 bg-red-500/10 text-red-700";
  return "border-slate-400/30 bg-slate-400/10 text-slate-600";
};
</script>

<template>
  <article class="flex h-full min-w-0 flex-col bg-background p-4">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0">
        <div class="truncate font-medium">
          {{ t(`admin.eventCenter.runtime.components.${component.id}`) }}
        </div>
        <div
          class="mt-1 truncate text-xs text-muted-foreground"
          :title="component.reason_code || '-'"
        >
          {{ component.reason_code || "-" }}
        </div>
      </div>
      <Badge
        variant="outline"
        class="shrink-0"
        :class="statusClass(component.status)"
      >
        {{ t(`admin.eventCenter.runtime.status.${component.status}`) }}
      </Badge>
    </div>

    <dl
      v-if="variant === 'process'"
      class="mt-4 grid grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)] gap-x-3 gap-y-2 text-sm"
    >
      <dt class="text-muted-foreground">
        {{ t("admin.eventCenter.runtime.process") }}
      </dt>
      <dd class="truncate text-right">
        {{
          t(`admin.eventCenter.runtime.processState.${component.process_state}`)
        }}
      </dd>
      <dt class="text-muted-foreground">
        {{ t("admin.eventCenter.runtime.version") }}
      </dt>
      <dd class="truncate text-right" :title="component.version || '-'">
        {{ component.version || "-" }}
      </dd>
      <dt class="text-muted-foreground">PID</dt>
      <dd class="truncate text-right tabular-nums">
        {{ component.pid ?? "-" }}
      </dd>
      <dt class="text-muted-foreground">
        {{ t("admin.eventCenter.runtime.startedAt") }}
      </dt>
      <dd class="truncate text-right" :title="formatDate(component.started_at)">
        {{ formatDate(component.started_at) }}
      </dd>
      <dt class="text-muted-foreground">
        {{ t("admin.eventCenter.runtime.uptime") }}
      </dt>
      <dd class="truncate text-right tabular-nums">
        {{ formatDuration(component.uptime_ms) }}
      </dd>
      <template v-if="component.rss_bytes != null">
        <dt class="text-muted-foreground">RSS</dt>
        <dd class="text-right tabular-nums">
          {{ formatBytes(component.rss_bytes) }}
        </dd>
      </template>
      <template v-if="component.heap_alloc_bytes != null">
        <dt class="text-muted-foreground">Go Heap</dt>
        <dd class="text-right tabular-nums">
          {{ formatBytes(component.heap_alloc_bytes) }}
        </dd>
      </template>
      <template v-if="component.goroutines != null">
        <dt class="text-muted-foreground">Goroutines</dt>
        <dd class="text-right tabular-nums">{{ component.goroutines }}</dd>
      </template>
    </dl>

    <dl
      v-else-if="component.latency_ms != null"
      class="mt-3 flex items-center justify-between gap-3 text-sm"
    >
      <dt class="text-muted-foreground">
        {{ t("admin.eventCenter.runtime.latency") }}
      </dt>
      <dd class="shrink-0 tabular-nums">{{ component.latency_ms }} ms</dd>
    </dl>

    <div
      v-if="showLogAction || showMemoryAction || showDebugAction"
      class="mt-auto flex flex-wrap items-center gap-2 pt-4"
    >
      <Button
        v-if="showLogAction"
        variant="outline"
        size="sm"
        @click="emit('viewLogs', props.component)"
      >
        <FileText class="mr-2 h-4 w-4" />
        {{ t("admin.eventCenter.runtime.viewLogs") }}
      </Button>
      <Button
        v-if="showDebugAction"
        variant="outline"
        size="icon-sm"
        :title="t('admin.eventCenter.runtime.debug.view')"
        :aria-label="t('admin.eventCenter.runtime.debug.view')"
        @click="emit('viewDebug', props.component)"
      >
        <Activity class="h-4 w-4" aria-hidden="true" />
      </Button>
      <Button
        v-if="showMemoryAction"
        variant="outline"
        size="icon-sm"
        :title="t('admin.eventCenter.runtime.memory.open')"
        :aria-label="t('admin.eventCenter.runtime.memory.open')"
        @click="emit('manageMemory', props.component)"
      >
        <MemoryStick class="h-4 w-4" />
      </Button>
    </div>
  </article>
</template>
