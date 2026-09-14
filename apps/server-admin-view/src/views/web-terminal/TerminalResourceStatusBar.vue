<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  CircleCheck,
  CircleDashed,
  CircleAlert,
  Cpu,
  MemoryStick,
  HardDrive,
  Clock3,
} from "lucide-vue-next";
import TerminalDiskPopover from "./TerminalDiskPopover.vue";
import { formatBytes, formatPercent } from "./terminal-metric-format";
import type { TerminalDisks, TerminalMetrics } from "@/lib/api/terminal";

const props = defineProps<{
  disks?: TerminalDisks | null;
  disksLoading?: boolean;
  disksFailed?: boolean;
  disksStale?: boolean;
  diskDetailsOpen?: boolean;
  metrics: TerminalMetrics | null;
  loading: boolean;
  failed: boolean;
  stale: boolean;
  connectionState: "idle" | "connecting" | "connected" | "error";
  connectionLabel: string;
}>();
const emit = defineEmits<{ "update:diskDetailsOpen": [open: boolean] }>();
const { t, locale } = useI18n();
const label = (key: string) => t(`admin.webTerminal.metrics.${key}`);
const bytes = (value: number) => formatBytes(value, locale.value);
const percent = (value: number | null | undefined) =>
  formatPercent(value, locale.value);
const uptime = computed(() => {
  const value = props.metrics?.uptime.value;
  if (value == null) return "—";
  const minutes = Math.floor(value / 60);
  return t("admin.webTerminal.metrics.duration", {
    days: Math.floor(minutes / 1440),
    hours: Math.floor(minutes / 60) % 24,
    minutes: minutes % 60,
  });
});
const items = computed(() => {
  const m = props.metrics;
  const memory = m?.memory;
  return [
    {
      key: "cpu",
      icon: Cpu,
      value: percent(m?.cpu.value),
      percent: m?.cpu.value,
      metric: m?.cpu,
    },
    {
      key: "memory",
      icon: MemoryStick,
      value:
        memory?.usedBytes != null && memory.totalBytes != null
          ? `${bytes(memory.usedBytes)} / ${bytes(memory.totalBytes)}`
          : "—",
      percent: memory?.percent,
      metric: memory,
    },
    {
      key: "disk",
      icon: HardDrive,
      value: percent(m?.disk.percent),
      percent: m?.disk.percent,
      metric: m?.disk,
    },
    {
      key: "uptime",
      icon: Clock3,
      value: uptime.value,
      percent: null,
      metric: m?.uptime,
    },
  ];
});
const reason = (item: (typeof items.value)[number]) => {
  const detail = item.metric?.reason;
  return [
    label(item.key),
    props.stale ? label("stale") : "",
    detail ? label(`reason.${detail}`) : "",
    item.key === "disk" ? label("rootFilesystem") : "",
  ]
    .filter(Boolean)
    .join(" · ");
};
const health = computed(() =>
  props.stale
    ? "stale"
    : props.failed
      ? "failed"
      : props.loading
        ? "loading"
        : "",
);
</script>

<template>
  <div
    class="terminal-resource-status shrink-0 border-t border-white/8 bg-[#252527] px-3 py-2 text-[11px] leading-4 text-white/55"
    :aria-label="label('title')"
    :aria-busy="loading"
    role="group"
  >
    <div
      class="metrics-list flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1.5"
    >
      <span
        class="metric-connection inline-flex shrink-0 items-center gap-1.5"
        :title="connectionLabel"
      >
        <CircleCheck
          v-if="connectionState === 'connected'"
          class="size-3 text-emerald-400/85"
          aria-hidden="true"
        />
        <CircleAlert
          v-else-if="connectionState === 'error'"
          class="size-3 text-amber-400/85"
          aria-hidden="true"
        />
        <CircleDashed
          v-else
          class="size-3"
          :class="{ 'animate-spin': connectionState === 'connecting' }"
          aria-hidden="true"
        />
        <span>{{ connectionLabel }}</span>
      </span>
      <span
        v-for="item in items"
        :key="item.key"
        class="inline-flex min-w-0 items-center gap-1.5"
        :title="item.key === 'disk' ? undefined : reason(item)"
        :data-metric="item.key"
      >
        <component
          :is="item.key === 'disk' ? TerminalDiskPopover : 'span'"
          class="inline-flex min-w-0 items-center gap-1.5"
          v-bind="
            item.key === 'disk'
              ? {
                  open: diskDetailsOpen ?? false,
                  disks: disks ?? null,
                  loading: disksLoading ?? false,
                  failed: disksFailed ?? false,
                  stale: disksStale ?? false,
                  disabled: connectionState !== 'connected',
                }
              : {}
          "
          @update:open="emit('update:diskDetailsOpen', $event)"
        >
          <component
            :is="item.icon"
            class="metric-icon size-3 shrink-0 text-white/35"
            aria-hidden="true"
          />
          <span class="metric-label">{{ label(item.key) }}</span>
          <span
            v-if="item.percent != null"
            class="metric-track hidden h-1 w-9 overflow-hidden rounded-full bg-white/10 sm:inline-block"
            aria-hidden="true"
          >
            <span
              class="block h-full rounded-full transition-[width] duration-300 motion-reduce:transition-none"
              :class="
                stale
                  ? 'bg-white/25'
                  : item.percent >= 90
                    ? 'bg-amber-400/80'
                    : 'bg-emerald-400/70'
              "
              :style="{ width: `${Math.min(100, Math.max(0, item.percent))}%` }"
            />
          </span>
          <span
            class="tabular-nums"
            :class="stale ? 'text-white/35' : 'text-white/80'"
            >{{ loading && !metrics ? "…" : item.value }}</span
          >
          <span
            v-if="item.metric?.status === 'estimated'"
            :aria-label="label('reason.estimated')"
            >≈</span
          >
          <span v-if="item.metric?.reason" class="sr-only">{{
            label(`reason.${item.metric.reason}`)
          }}</span>
        </component>
      </span>
      <span
        v-if="health"
        class="inline-flex items-center gap-1 text-amber-300/80"
        role="status"
      >
        <CircleAlert
          v-if="health !== 'loading'"
          class="size-3"
          aria-hidden="true"
        />
        {{ label(health) }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.terminal-resource-status {
  container-type: inline-size;
}
@container (max-width: 640px) {
  .metrics-list {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    column-gap: 8px;
  }
  .metric-connection {
    grid-column: 1 / 3;
    grid-row: 1;
  }
  [data-metric="cpu"] {
    grid-column: 3 / 5;
    grid-row: 1;
  }
  [data-metric="disk"] {
    grid-column: 5 / 7;
    grid-row: 1;
  }
  [data-metric="memory"] {
    grid-column: 1 / 4;
    grid-row: 2;
  }
  [data-metric="uptime"] {
    grid-column: 4 / 7;
    grid-row: 2;
    justify-self: end;
  }
  [role="status"] {
    grid-column: 1 / -1;
  }
  .metric-track,
  [data-metric="cpu"] .metric-icon,
  [data-metric="disk"] .metric-icon {
    display: none;
  }
  [data-metric="memory"] .metric-label,
  [data-metric="uptime"] .metric-label {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
}
</style>
