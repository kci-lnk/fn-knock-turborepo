<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Copy, Download, Loader2, RefreshCw, Trash2 } from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { toast } from "vue-sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { EventCenterAPI, RuntimeHealthAPI } from "@/lib/api";
import RuntimeComponentCard from "./RuntimeComponentCard.vue";
import type {
  RuntimeComponentHealth,
  RuntimeLogComponent,
  RuntimeOperationalLogEntry,
  RuntimeHealthSnapshot,
  RuntimeHealthStatus,
  SystemEventRecord,
} from "../../types";

const props = withDefaults(defineProps<{ active?: boolean }>(), {
  active: true,
});
const { t } = useI18n();
const snapshot = ref<RuntimeHealthSnapshot | null>(null);
const recentEvents = ref<SystemEventRecord[]>([]);
const loading = ref(false);
const exporting = ref(false);
const copying = ref(false);
const logDialogOpen = ref(false);
const logComponent = ref<RuntimeLogComponent | null>(null);
const logEntries = ref<RuntimeOperationalLogEntry[]>([]);
const logGeneratedAt = ref<string | null>(null);
const logsLoading = ref(false);
const logsClearing = ref(false);
let pollTimer: ReturnType<typeof setInterval> | null = null;
let logRequestId = 0;

const componentOrder = [
  "management",
  "gateway_process",
  "gateway_dataplane",
  "auth_bridge",
  "storage",
  "config_sync",
] as const;

const components = computed(() =>
  componentOrder
    .map((id) => snapshot.value?.components[id])
    .filter((component): component is RuntimeComponentHealth => !!component),
);

const hasProcessDetails = (component: RuntimeComponentHealth) =>
  component.process_state !== "not_applicable";

const processComponents = computed(() =>
  components.value.filter(hasProcessDetails),
);

const serviceComponents = computed(() =>
  components.value.filter((component) => !hasProcessDetails(component)),
);

const isLogComponent = (
  component: RuntimeComponentHealth,
): component is RuntimeComponentHealth & { id: RuntimeLogComponent } =>
  component.id === "management" || component.id === "gateway_process";

const selectedLogComponentName = computed(() =>
  logComponent.value
    ? t(`admin.eventCenter.runtime.components.${logComponent.value}`)
    : "",
);

const fetchRuntime = async (showError = true) => {
  if (loading.value) return;
  loading.value = true;
  try {
    const [health, events] = await Promise.all([
      RuntimeHealthAPI.getHealth(),
      EventCenterAPI.getEvents({
        page: 1,
        limit: "20",
        search: "",
        source: "RUNTIME_MONITOR",
      }),
    ]);
    snapshot.value = health.data;
    recentEvents.value = events.data.events;
  } catch (error) {
    if (showError) {
      toast.error(t("admin.eventCenter.runtime.loadFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  } finally {
    loading.value = false;
  }
};

const stopPolling = () => {
  if (pollTimer) clearInterval(pollTimer);
  pollTimer = null;
};

const syncPolling = () => {
  stopPolling();
  if (!props.active || document.hidden) return;
  void fetchRuntime(snapshot.value === null);
  pollTimer = setInterval(() => void fetchRuntime(false), 5_000);
};

const copyDiagnostics = async () => {
  copying.value = true;
  try {
    const result = await RuntimeHealthAPI.getDiagnostics();
    await navigator.clipboard.writeText(JSON.stringify(result.data, null, 2));
    toast.success(t("admin.eventCenter.runtime.copySuccess"));
  } catch (error) {
    toast.error(t("admin.eventCenter.runtime.copyFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    copying.value = false;
  }
};

const exportDiagnostics = async () => {
  exporting.value = true;
  try {
    const { blob, filename } = await RuntimeHealthAPI.downloadArchive();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = filename;
    anchor.click();
    URL.revokeObjectURL(url);
    toast.success(t("admin.eventCenter.runtime.exportSuccess"));
  } catch (error) {
    toast.error(t("admin.eventCenter.runtime.exportFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    exporting.value = false;
  }
};

const loadRuntimeLogs = async () => {
  const component = logComponent.value;
  if (!component || logsLoading.value) return;
  const requestId = ++logRequestId;
  logsLoading.value = true;
  try {
    const result = await RuntimeHealthAPI.getLogs(component);
    if (requestId !== logRequestId) return;
    logEntries.value = result.data.entries;
    logGeneratedAt.value = result.data.generated_at;
  } catch (error) {
    if (requestId !== logRequestId) return;
    toast.error(t("admin.eventCenter.runtime.logLoadFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    if (requestId === logRequestId) logsLoading.value = false;
  }
};

const openRuntimeLogs = (component: RuntimeComponentHealth) => {
  if (!isLogComponent(component)) return;
  logComponent.value = component.id;
  logEntries.value = [];
  logGeneratedAt.value = null;
  logDialogOpen.value = true;
  void loadRuntimeLogs();
};

const clearRuntimeLogs = async () => {
  const component = logComponent.value;
  if (!component || logsClearing.value) return;
  logsClearing.value = true;
  ++logRequestId;
  try {
    const result = await RuntimeHealthAPI.clearLogs(component);
    logEntries.value = [];
    logGeneratedAt.value = result.data.cleared_at;
    toast.success(t("admin.eventCenter.runtime.logClearSuccess"));
    void fetchRuntime(false);
  } catch (error) {
    toast.error(t("admin.eventCenter.runtime.logClearFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  } finally {
    logsClearing.value = false;
    logsLoading.value = false;
  }
};

const formatDate = (value?: string | null) =>
  value ? new Date(value).toLocaleString() : "-";

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

const eventComponent = (event: SystemEventRecord) =>
  String(event.payload?.component || event.subject?.id || "-");

const formatLogLine = (entry: RuntimeOperationalLogEntry) => {
  const fields =
    entry.fields && Object.keys(entry.fields).length
      ? ` ${JSON.stringify(entry.fields)}`
      : "";
  return `${entry.time} [${entry.level}] ${entry.component}/${entry.event}${
    entry.reason_code ? ` (${entry.reason_code})` : ""
  }${fields}`;
};

watch(() => props.active, syncPolling);
onMounted(() => {
  document.addEventListener("visibilitychange", syncPolling);
  syncPolling();
});
onUnmounted(() => {
  stopPolling();
  document.removeEventListener("visibilitychange", syncPolling);
});
</script>

<template>
  <div class="flex h-full flex-col gap-4 overflow-auto pb-2">
    <div
      class="flex flex-col items-stretch justify-between gap-4 rounded-lg border bg-background p-4 lg:flex-row lg:items-center"
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2">
          <span class="text-base font-semibold">{{
            t("admin.eventCenter.runtime.overall")
          }}</span>
          <Badge
            v-if="snapshot"
            variant="outline"
            :class="statusClass(snapshot.overall_status)"
          >
            {{
              t(`admin.eventCenter.runtime.status.${snapshot.overall_status}`)
            }}
          </Badge>
          <Loader2
            v-else-if="loading"
            class="h-4 w-4 animate-spin text-muted-foreground"
          />
        </div>
        <div class="text-sm text-muted-foreground">
          {{ t("admin.eventCenter.runtime.lastChecked") }}:
          {{ formatDate(snapshot?.last_checked_at) }}
          <span v-if="snapshot">
            · {{ t("admin.eventCenter.runtime.supervisor") }}:
            {{ snapshot.supervisor }}</span
          >
        </div>
      </div>
      <div class="grid w-full grid-cols-1 gap-2 sm:grid-cols-3 lg:w-auto">
        <Button
          variant="outline"
          size="sm"
          :disabled="loading"
          @click="fetchRuntime()"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': loading }"
          />
          {{ t("admin.eventCenter.runtime.refresh") }}
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="copying"
          @click="copyDiagnostics"
        >
          <Loader2 v-if="copying" class="mr-2 h-4 w-4 animate-spin" />
          <Copy v-else class="mr-2 h-4 w-4" />
          {{ t("admin.eventCenter.runtime.copy") }}
        </Button>
        <Button size="sm" :disabled="exporting" @click="exportDiagnostics">
          <Loader2 v-if="exporting" class="mr-2 h-4 w-4 animate-spin" />
          <Download v-else class="mr-2 h-4 w-4" />
          {{ t("admin.eventCenter.runtime.export") }}
        </Button>
      </div>
    </div>

    <div class="grid gap-4 xl:grid-cols-3">
      <section
        class="flex flex-col overflow-hidden rounded-lg border xl:col-span-2"
      >
        <div class="border-b bg-muted/20 px-4 py-3">
          <h3 class="font-medium">
            {{ t("admin.eventCenter.runtime.processSection") }}
          </h3>
        </div>
        <div class="grid flex-1 gap-px bg-border md:grid-cols-2">
          <RuntimeComponentCard
            v-for="component in processComponents"
            :key="component.id"
            :component="component"
            variant="process"
            show-log-action
            @view-logs="openRuntimeLogs"
          />
        </div>
      </section>

      <section
        class="flex flex-col overflow-hidden rounded-lg border xl:col-span-1"
      >
        <div class="border-b bg-muted/20 px-4 py-3">
          <h3 class="font-medium">
            {{ t("admin.eventCenter.runtime.serviceSection") }}
          </h3>
        </div>
        <div
          class="grid flex-1 gap-px bg-border sm:grid-cols-2 xl:auto-rows-fr xl:grid-cols-1"
        >
          <RuntimeComponentCard
            v-for="component in serviceComponents"
            :key="component.id"
            :component="component"
            variant="service"
          />
        </div>
      </section>
    </div>

    <div v-if="snapshot" class="rounded-lg border bg-background p-4">
      <div class="mb-3 font-medium">
        {{ t("admin.eventCenter.runtime.logs") }}
      </div>
      <div class="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.coverage")
          }}</span>
          <div>
            {{ formatDate(snapshot.logs.oldest_at) }} —
            {{ formatDate(snapshot.logs.newest_at) }}
          </div>
        </div>
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.diskUsage")
          }}</span>
          <div>{{ formatBytes(snapshot.logs.bytes_used) }} / 6 MiB</div>
        </div>
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.dropped")
          }}</span>
          <div>{{ snapshot.logs.dropped_info }}</div>
        </div>
        <div>
          <span class="text-muted-foreground">{{
            t("admin.eventCenter.runtime.directory")
          }}</span>
          <div>{{ snapshot.logs.directory }}</div>
        </div>
      </div>
    </div>

    <div class="rounded-lg border bg-background">
      <div class="border-b px-4 py-3 font-medium">
        {{ t("admin.eventCenter.runtime.recentEvents") }}
      </div>
      <div v-if="recentEvents.length" class="divide-y">
        <div
          v-for="event in recentEvents"
          :key="event.id"
          class="flex flex-wrap items-center gap-2 px-4 py-3 text-sm"
        >
          <Badge variant="outline">{{
            t(`admin.eventCenter.eventTypes.${event.type}`)
          }}</Badge>
          <span class="font-medium">{{
            t(`admin.eventCenter.runtime.components.${eventComponent(event)}`)
          }}</span>
          <span class="text-muted-foreground">{{
            String(event.payload?.reason_code || "-")
          }}</span>
          <span class="ml-auto text-xs text-muted-foreground">{{
            formatDate(event.happened_at)
          }}</span>
        </div>
      </div>
      <div v-else class="px-4 py-8 text-center text-sm text-muted-foreground">
        {{ t("admin.eventCenter.runtime.noEvents") }}
      </div>
    </div>

    <Dialog v-model:open="logDialogOpen">
      <DialogContent class="flex max-h-[85vh] flex-col sm:max-w-4xl">
        <DialogHeader class="shrink-0 pr-8 text-left">
          <DialogTitle>
            {{
              t("admin.eventCenter.runtime.logDialogTitle", {
                component: selectedLogComponentName,
              })
            }}
          </DialogTitle>
          <DialogDescription>
            {{ t("admin.eventCenter.runtime.logDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div
          class="flex shrink-0 flex-col items-stretch justify-between gap-2 text-xs sm:flex-row sm:items-center"
        >
          <span class="text-muted-foreground">
            {{ t("admin.eventCenter.runtime.logUpdatedAt") }}:
            {{ formatDate(logGeneratedAt) }}
          </span>
          <div class="grid grid-cols-2 gap-2 sm:flex">
            <ConfirmDangerPopover
              :title="t('admin.eventCenter.runtime.clearLogTitle')"
              :description="
                t('admin.eventCenter.runtime.clearLogDescription', {
                  component: selectedLogComponentName,
                })
              "
              :confirm-text="t('admin.eventCenter.runtime.confirmClearLogs')"
              :loading="logsClearing"
              :disabled="logsLoading || logsClearing"
              content-class="w-80 text-left"
              :on-confirm="clearRuntimeLogs"
            >
              <template #trigger>
                <Button
                  variant="outline"
                  size="sm"
                  class="border-destructive/20 text-destructive hover:bg-destructive/5 hover:text-destructive"
                  :disabled="logsLoading || logsClearing"
                >
                  <Trash2 class="mr-2 h-4 w-4" />
                  {{ t("admin.eventCenter.runtime.clearLogs") }}
                </Button>
              </template>
            </ConfirmDangerPopover>
            <Button
              variant="outline"
              size="sm"
              :disabled="logsLoading || logsClearing"
              @click="loadRuntimeLogs"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="{ 'animate-spin': logsLoading }"
              />
              {{ t("admin.eventCenter.runtime.refresh") }}
            </Button>
          </div>
        </div>

        <div
          class="min-h-48 flex-1 overflow-auto rounded-md border bg-slate-950 p-3 font-mono text-xs leading-5 text-slate-100"
        >
          <div
            v-if="logsLoading && !logEntries.length"
            class="flex h-48 items-center justify-center text-slate-400"
          >
            <Loader2 class="mr-2 h-4 w-4 animate-spin" />
            {{ t("admin.eventCenter.runtime.loadingLogs") }}
          </div>
          <div
            v-else-if="!logEntries.length"
            class="flex h-48 items-center justify-center text-slate-400"
          >
            {{ t("admin.eventCenter.runtime.noLogs") }}
          </div>
          <div v-else class="space-y-1">
            <div
              v-for="(entry, index) in logEntries"
              :key="`${entry.time}-${entry.event}-${index}`"
              class="whitespace-pre-wrap break-all"
            >
              {{ formatLogLine(entry) }}
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>
