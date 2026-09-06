<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Copy,
  Download,
  Loader2,
  Play,
  RefreshCw,
  Square,
} from "lucide-vue-next";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useRuntimeDebug } from "./useRuntimeDebug";
import {
  formatRuntimeBytes as bytes,
  formatRuntimeDate as date,
} from "./runtimePresentation";
import {
  formatDebugPercent as percent,
  summarizeDebugSamples,
} from "./runtimeDebugPresentation";

const props = withDefaults(defineProps<{ open: boolean; active?: boolean }>(), {
  active: true,
});
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { t } = useI18n();
const key = (name: string) => `admin.eventCenter.runtime.debug.${name}`;
const categoryLabel = (category: string) =>
  ["heap", "main_stack", "anonymous_mappings", "file_or_special"].includes(
    category,
  )
    ? t(key(`categories.${category}`))
    : category;
const {
  report,
  loading,
  action,
  error,
  unavailable,
  running,
  remainingSeconds,
  refresh,
  start,
  stop,
  refreshMemory,
} = useRuntimeDebug({ enabled: () => props.open && props.active });
const copying = ref(false);
const latest = computed(() => {
  const samples = report.value?.capture.samples;
  return samples?.[samples.length - 1];
});
const summary = computed(() =>
  summarizeDebugSamples(report.value?.capture.samples ?? []),
);
const operations = computed(() =>
  [...(report.value?.capture.operations.operations ?? [])].sort(
    (left, right) => right.total_wall_ms - left.total_wall_ms,
  ),
);
const operationGroups = computed(() => [
  {
    name: "tasks",
    items: operations.value.filter((item) => item.kind === "task").slice(0, 10),
  },
  {
    name: "waits",
    items: operations.value.filter((item) => item.kind === "wait").slice(0, 10),
  },
  {
    name: "sqlite",
    items: operations.value
      .filter((item) => item.kind !== "task" && item.kind !== "wait")
      .slice(0, 10),
  },
]);
const memoryRows = computed(() =>
  report.value?.memory
    ? [
        { name: "snapshotRss", value: report.value.memory.rss_bytes },
        { name: "anonymous", value: report.value.memory.anonymous_bytes },
        { name: "file", value: report.value.memory.file_bytes },
        { name: "swap", value: report.value.memory.swap_bytes },
      ]
    : [],
);
const allocatorRows = computed(() => {
  const allocator = report.value?.memory?.allocator;
  return allocator
    ? [
        { name: "allocated", value: allocator.allocated_bytes },
        { name: "free", value: allocator.free_bytes },
        { name: "mapped", value: allocator.mmap_bytes },
        { name: "releasable", value: allocator.releasable_bytes },
      ]
    : [];
});
const copy = async () => {
  if (!report.value || copying.value) return;
  copying.value = true;
  try {
    await navigator.clipboard.writeText(JSON.stringify(report.value, null, 2));
    toast.success(t(key("copied")));
  } catch {
    toast.error(t(key("copyFailed")));
  } finally {
    copying.value = false;
  }
};
const download = () => {
  if (!report.value) return;
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(report.value, null, 2)], {
      type: "application/json",
    }),
  );
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `fn-knock-runtime-debug-${report.value.process.pid}-${Date.now()}.json`;
  anchor.click();
  URL.revokeObjectURL(url);
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[90dvh] flex-col gap-4 overflow-hidden sm:max-w-5xl"
    >
      <DialogHeader class="shrink-0 pr-6 text-left">
        <DialogTitle>{{ t(key("title")) }}</DialogTitle>
        <DialogDescription>{{ t(key("description")) }}</DialogDescription>
      </DialogHeader>
      <div class="min-h-0 space-y-4 overflow-y-auto pr-1">
        <div
          v-if="error"
          role="alert"
          class="flex items-center justify-between gap-3 rounded-md border border-destructive/30 p-3 text-sm text-destructive"
        >
          <span>{{ t(key(unavailable ? "unavailable" : "loadFailed")) }}</span>
          <Button
            variant="outline"
            size="sm"
            :disabled="loading"
            @click="refresh"
            >{{ t(key("retry")) }}</Button
          >
        </div>
        <div
          v-if="!report && loading"
          class="flex justify-center gap-2 py-10 text-sm text-muted-foreground"
        >
          <Loader2 class="h-4 w-4 animate-spin" />{{ t(key("loading")) }}
        </div>
        <template v-if="report">
          <div
            class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground"
          >
            <span
              >Rust {{ report.process.version }} · PID
              {{ report.process.pid }}</span
            >
            <span>{{ report.process.os }} / {{ report.process.arch }}</span>
            <span>{{
              t(key("cpus"), { count: report.process.logical_cpus })
            }}</span>
            <span
              >{{ t(key("updated")) }}: {{ date(report.generated_at) }}</span
            >
          </div>
          <section class="space-y-3 rounded-lg border p-3 sm:p-4">
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
            >
              <div class="flex flex-wrap items-center gap-2" aria-live="polite">
                <h3 class="text-sm font-semibold">{{ t(key("capture")) }}</h3>
                <Badge variant="outline">{{
                  t(key(`status.${report.capture.status}`))
                }}</Badge>
                <span v-if="running" class="text-sm tabular-nums">{{
                  t(key("remaining"), { seconds: remainingSeconds })
                }}</span>
                <span class="text-xs text-muted-foreground">{{
                  t(key("samples"), { count: report.capture.samples.length })
                }}</span>
              </div>
              <div class="flex flex-wrap gap-2">
                <Button
                  v-if="running"
                  size="sm"
                  variant="outline"
                  :disabled="!!action || unavailable"
                  @click="stop"
                >
                  <Loader2
                    v-if="action === 'stop'"
                    class="h-4 w-4 animate-spin"
                  /><Square v-else class="h-4 w-4" />{{ t(key("stop")) }}
                </Button>
                <Button
                  v-else
                  size="sm"
                  :disabled="!!action || unavailable"
                  @click="start"
                >
                  <Loader2
                    v-if="action === 'start'"
                    class="h-4 w-4 animate-spin"
                  /><Play v-else class="h-4 w-4" />{{ t(key("start")) }}
                </Button>
              </div>
            </div>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t(key("captureHint")) }}
            </p>
            <p
              v-if="report.capture.started_at"
              class="text-xs text-muted-foreground"
            >
              {{ t(key("captureWindow")) }}:
              {{ date(report.capture.started_at) }} —
              {{
                report.capture.finished_at
                  ? date(report.capture.finished_at)
                  : t(key("status.running"))
              }}
            </p>
            <div v-if="latest" class="grid grid-cols-2 gap-3 md:grid-cols-4">
              <div>
                <div class="text-xs text-muted-foreground">
                  {{ t(key("averageCpu")) }}
                </div>
                <div class="mt-1 text-lg font-semibold tabular-nums">
                  {{ percent(summary.averageCpu) }}
                </div>
              </div>
              <div>
                <div class="text-xs text-muted-foreground">
                  {{ t(key("peakCpu")) }}
                </div>
                <div class="mt-1 text-lg font-semibold tabular-nums">
                  {{ percent(summary.maxCpu) }}
                </div>
              </div>
              <div>
                <div class="text-xs text-muted-foreground">
                  {{ t(key("latestRss")) }}
                </div>
                <div class="mt-1 text-lg font-semibold tabular-nums">
                  {{ bytes(latest.resource.rss_bytes) }}
                </div>
              </div>
              <div>
                <div class="text-xs text-muted-foreground">
                  {{ t(key("rssChange")) }}
                </div>
                <div class="mt-1 text-lg font-semibold tabular-nums">
                  {{
                    summary.rssDelta == null
                      ? "—"
                      : `${summary.rssDelta > 0 ? "+" : summary.rssDelta < 0 ? "−" : ""}${bytes(Math.abs(summary.rssDelta))}`
                  }}
                </div>
              </div>
            </div>
            <p
              v-else
              class="rounded-md bg-muted/50 p-3 text-sm text-muted-foreground"
            >
              {{ t(key("empty")) }}
            </p>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t(key("cpuHint")) }} {{ t(key("rssHint")) }}
            </p>
            <p
              v-if="report.capture.errors.length"
              class="text-xs text-amber-700"
            >
              {{ t(key("partial")) }}: {{ report.capture.errors.join(", ") }}
            </p>
            <p
              v-if="latest?.resource.errors.length"
              class="text-xs text-amber-700"
            >
              {{ t(key("partial")) }}: {{ latest.resource.errors.join(", ") }}
            </p>
            <details v-if="report.capture.samples.length" class="text-xs">
              <summary class="cursor-pointer py-1">
                {{ t(key("sampleDetails")) }}
              </summary>
              <div class="mt-2 max-h-52 overflow-auto">
                <table class="w-full text-left tabular-nums">
                  <thead>
                    <tr class="border-b text-muted-foreground">
                      <th class="p-2">{{ t(key("time")) }}</th>
                      <th class="p-2">CPU</th>
                      <th class="p-2">RSS</th>
                      <th class="p-2">{{ t(key("queue")) }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="sample in report.capture.samples"
                      :key="sample.elapsed_ms"
                      class="border-b"
                    >
                      <td class="whitespace-nowrap p-2">
                        {{ date(sample.at) }}
                      </td>
                      <td class="p-2">
                        {{ percent(sample.resource.cpu_percent) }}
                      </td>
                      <td class="whitespace-nowrap p-2">
                        {{ bytes(sample.resource.rss_bytes) }}
                      </td>
                      <td class="p-2">{{ sample.queue_depth }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </details>
          </section>
          <section class="space-y-3 rounded-lg border p-3 sm:p-4">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <h3 class="text-sm font-semibold">{{ t(key("memory")) }}</h3>
              <Button
                variant="outline"
                size="sm"
                :disabled="!!action || report.memory_refreshing || unavailable"
                @click="refreshMemory"
                ><Loader2
                  v-if="action === 'memory' || report.memory_refreshing"
                  class="h-4 w-4 animate-spin"
                /><RefreshCw v-else class="h-4 w-4" />{{
                  t(key("refreshMemory"))
                }}</Button
              >
            </div>
            <p class="text-xs text-muted-foreground">
              {{ t(key("memoryHint")) }}
            </p>
            <template v-if="report.memory">
              <p class="text-xs text-muted-foreground">
                {{ date(report.memory.collected_at) }} ·
                {{ t(key(`memoryStatus.${report.memory.status}`)) }}
              </p>
              <dl class="grid grid-cols-2 gap-3 sm:grid-cols-4">
                <div v-for="row in memoryRows" :key="row.name">
                  <dt class="text-xs text-muted-foreground">
                    {{ t(key(row.name)) }}
                  </dt>
                  <dd class="mt-1 text-sm tabular-nums">
                    {{ bytes(row.value) }}
                  </dd>
                </div>
              </dl>
              <div v-if="report.memory.categories.length" class="overflow-auto">
                <table class="w-full text-left text-xs tabular-nums">
                  <thead>
                    <tr class="border-b text-muted-foreground">
                      <th class="p-2">{{ t(key("category")) }}</th>
                      <th class="p-2">RSS</th>
                      <th class="p-2">PSS</th>
                      <th class="p-2">{{ t(key("mappings")) }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="row in report.memory.categories"
                      :key="row.category"
                      class="border-b"
                    >
                      <td class="p-2">{{ categoryLabel(row.category) }}</td>
                      <td class="whitespace-nowrap p-2">
                        {{ bytes(row.rss_bytes) }}
                      </td>
                      <td class="whitespace-nowrap p-2">
                        {{ bytes(row.pss_bytes) }}
                      </td>
                      <td class="p-2">{{ row.mappings }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <details v-if="allocatorRows.length" class="text-xs">
                <summary class="cursor-pointer py-1">
                  {{ t(key("allocator")) }}
                </summary>
                <p class="my-2 leading-5 text-muted-foreground">
                  {{ t(key("allocatorHint")) }}
                </p>
                <dl class="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  <div v-for="row in allocatorRows" :key="row.name">
                    <dt class="text-muted-foreground">
                      {{ t(key(row.name)) }}
                    </dt>
                    <dd class="mt-1 tabular-nums">{{ bytes(row.value) }}</dd>
                  </div>
                </dl>
              </details>
              <details
                v-if="report.memory.largest_anonymous_regions.length"
                class="text-xs"
              >
                <summary class="cursor-pointer py-1">
                  {{ t(key("largestRegions")) }}
                </summary>
                <div class="mt-2 overflow-auto">
                  <table class="w-full text-left tabular-nums">
                    <thead>
                      <tr class="border-b text-muted-foreground">
                        <th class="p-2">{{ t(key("category")) }}</th>
                        <th class="p-2">{{ t(key("size")) }}</th>
                        <th class="p-2">RSS</th>
                        <th class="p-2">{{ t(key("anonymous")) }}</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr
                        v-for="(region, index) in report.memory
                          .largest_anonymous_regions"
                        :key="index"
                        class="border-b"
                      >
                        <td class="p-2">
                          {{ categoryLabel(region.category) }}
                        </td>
                        <td class="whitespace-nowrap p-2">
                          {{ bytes(region.size_bytes) }}
                        </td>
                        <td class="whitespace-nowrap p-2">
                          {{ bytes(region.rss_bytes) }}
                        </td>
                        <td class="whitespace-nowrap p-2">
                          {{ bytes(region.anonymous_bytes) }}
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </details>
              <p
                v-if="report.memory.errors.length"
                class="text-xs text-amber-700"
              >
                {{ t(key("partial")) }}: {{ report.memory.errors.join(", ") }}
              </p>
            </template>
            <p v-else class="text-sm text-muted-foreground">
              {{ t(key("memoryEmpty")) }}
            </p>
          </section>
          <section class="space-y-3 rounded-lg border p-3 sm:p-4">
            <h3 class="text-sm font-semibold">{{ t(key("threads")) }}</h3>
            <p class="text-xs text-muted-foreground">
              {{ t(key("threadsHint")) }}
            </p>
            <div v-if="summary.threads.length" class="overflow-auto">
              <table class="w-full text-left text-xs tabular-nums">
                <thead>
                  <tr class="border-b text-muted-foreground">
                    <th class="p-2">TID</th>
                    <th class="p-2">{{ t(key("name")) }}</th>
                    <th class="p-2">{{ t(key("averageCpu")) }}</th>
                    <th class="p-2">{{ t(key("peakCpu")) }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="thread in summary.threads"
                    :key="thread.tid"
                    class="border-b"
                  >
                    <td class="p-2">{{ thread.tid }}</td>
                    <td class="p-2">{{ thread.name }}</td>
                    <td class="p-2">{{ percent(thread.average) }}</td>
                    <td class="p-2">{{ percent(thread.peak) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <p v-else class="text-sm text-muted-foreground">
              {{ t(key("threadsEmpty")) }}
            </p>
          </section>
          <section class="space-y-3 rounded-lg border p-3 sm:p-4">
            <h3 class="text-sm font-semibold">{{ t(key("operations")) }}</h3>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t(key("wallHint")) }}
            </p>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t(key("rowsHint")) }}
            </p>
            <div v-for="group in operationGroups" :key="group.name">
              <h4 class="mb-2 text-xs font-semibold">
                {{ t(key(group.name)) }}
              </h4>
              <div v-if="group.items.length" class="overflow-auto">
                <table
                  class="w-full min-w-[48rem] text-left text-xs tabular-nums [&_th]:whitespace-nowrap"
                >
                  <thead>
                    <tr class="border-b text-muted-foreground">
                      <th class="p-2">{{ t(key("operation")) }}</th>
                      <th class="p-2">{{ t(key("calls")) }}</th>
                      <th class="p-2">{{ t(key("wall")) }}</th>
                      <th class="p-2">{{ t(key("maxWall")) }}</th>
                      <th class="p-2">CPU ms</th>
                      <th class="p-2">{{ t(key("failures")) }}</th>
                      <th class="p-2">{{ t(key("inFlight")) }}</th>
                      <th class="p-2">{{ t(key("rows")) }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="item in group.items"
                      :key="`${item.kind}:${item.label}`"
                      class="border-b"
                    >
                      <td class="max-w-64 break-words p-2">
                        <div>{{ item.label }}</div>
                        <div class="text-muted-foreground">{{ item.kind }}</div>
                      </td>
                      <td class="p-2">{{ item.calls }}</td>
                      <td class="p-2">{{ item.total_wall_ms.toFixed(1) }}</td>
                      <td class="p-2">{{ item.max_wall_ms.toFixed(1) }}</td>
                      <td class="p-2">
                        {{ item.total_cpu_ms?.toFixed(1) ?? "—" }}
                      </td>
                      <td class="p-2">
                        {{ item.failures }} / {{ item.cancelled }}
                      </td>
                      <td class="p-2">{{ item.in_flight }}</td>
                      <td class="p-2">{{ item.rows ?? "—" }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <p v-else class="text-xs text-muted-foreground">
                {{ t(key("operationsEmpty")) }}
              </p>
            </div>
            <p
              v-if="report.capture.operations.dropped_operations"
              class="text-xs text-amber-700"
            >
              {{
                t(key("dropped"), {
                  count: report.capture.operations.dropped_operations,
                })
              }}
            </p>
            <dl
              class="grid grid-cols-2 gap-3 border-t pt-3 text-xs sm:grid-cols-3"
            >
              <div>
                <dt class="text-muted-foreground">
                  {{ t(key("queueDepth")) }}
                </dt>
                <dd class="mt-1 tabular-nums">
                  {{ report.queue.queue_depth }} /
                  {{ report.queue.queue_depth_peak }}
                </dd>
              </div>
              <div>
                <dt class="text-muted-foreground">{{ t(key("queueWait")) }}</dt>
                <dd class="mt-1 tabular-nums">
                  {{ report.queue.queue_wait_ms }} /
                  {{ report.queue.queue_wait_peak_ms }} ms
                </dd>
              </div>
              <div>
                <dt class="text-muted-foreground">
                  {{ t(key("activeOperation")) }}
                </dt>
                <dd class="mt-1 tabular-nums">
                  {{ report.queue.active_operation_ms }} ms
                </dd>
              </div>
            </dl>
          </section>
        </template>
      </div>
      <div
        class="flex shrink-0 flex-col gap-2 border-t pt-3 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-xs text-muted-foreground">{{ t(key("privacy")) }}</p>
        <div class="flex shrink-0 gap-2">
          <Button
            variant="outline"
            size="sm"
            :disabled="!report || copying"
            @click="copy"
            ><Copy class="h-4 w-4" />{{ t(key("copy")) }}</Button
          ><Button
            variant="outline"
            size="sm"
            :disabled="!report"
            @click="download"
            ><Download class="h-4 w-4" />{{ t(key("export")) }}</Button
          >
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
