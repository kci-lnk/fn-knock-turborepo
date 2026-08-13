<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { Activity, Download, Eraser, Square } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { DeepMonitorAPI } from "@/lib/api/deep-monitor";
import type { DeepMonitorEventSummary, DeepMonitorSession } from "@/types";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";

const DEFAULT_DURATION_SECONDS = 30 * 60;
const LOG_LINE_LIMIT = 1000;
const LOG_LINE_CHARACTER_LIMIT = 2048;

const { t, te } = useI18n();
const route = useRoute();
const host = computed(() =>
  typeof route.params.host === "string" ? route.params.host.trim() : "",
);
const session = ref<DeepMonitorSession | null>(null);
const logLines = ref<string[]>([]);
const lastSequence = ref(0);
const discardedLines = ref(0);
const mutating = ref(false);
const loading = ref(false);
const now = ref(Date.now());
const logViewport = ref<HTMLElement | null>(null);
let source: EventSource | null = null;
let clock: number | undefined;

const normalizeHost = (value: string) =>
  value.trim().toLowerCase().replace(/\.+$/, "");
const active = computed(() => session.value?.state === "active");
const stateLabel = computed(() => {
  const value = session.value?.state;
  if (!value) return t("admin.deepMonitor.ready");
  const key = `admin.deepMonitor.states.${value}`;
  return te(key) ? t(key) : value;
});
const remaining = computed(() => {
  if (!active.value) return "";
  const deadline = Date.parse(session.value?.deadline_at || "");
  if (!Number.isFinite(deadline)) return "";
  const seconds = Math.max(0, Math.floor((deadline - now.value) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
});
const logText = computed(() => logLines.value.join("\n"));

const formatBytes = (value: number) => {
  if (!value) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  const unit = Math.min(
    Math.floor(Math.log(value) / Math.log(1024)),
    units.length - 1,
  );
  return `${(value / 1024 ** unit).toFixed(unit ? 1 : 0)} ${units[unit]}`;
};

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : t("admin.deepMonitor.requestFailed");

const cleanLogValue = (value: string | number | undefined) =>
  String(value ?? "")
    .replace(/[\r\n\t]+/g, " ")
    .trim();

const formatLogLine = (item: DeepMonitorEventSummary) => {
  const request = [item.method, item.path]
    .map(cleanLogValue)
    .filter(Boolean)
    .join(" ");
  const fields = [
    `[${cleanLogValue(item.time) || new Date().toISOString()}]`,
    cleanLogValue(item.type),
    item.direction ? `direction=${cleanLogValue(item.direction)}` : "",
    item.client_ip ? `ip=${cleanLogValue(item.client_ip)}` : "",
    item.identity ? `identity=${cleanLogValue(item.identity)}` : "",
    request,
    item.status ? `status=${item.status}` : "",
    item.payload_bytes ? `bytes=${item.payload_bytes}` : "",
    cleanLogValue(item.notice),
  ].filter(Boolean);
  const line = fields.join(" ");
  return line.length > LOG_LINE_CHARACTER_LIMIT
    ? `${line.slice(0, LOG_LINE_CHARACTER_LIMIT)}…`
    : line;
};

const scrollToLatest = () => {
  void nextTick(() => {
    const element = logViewport.value;
    if (element) element.scrollTop = element.scrollHeight;
  });
};

const appendEvent = (item: DeepMonitorEventSummary, autoScroll = true) => {
  if (item.sequence <= lastSequence.value) return;
  lastSequence.value = item.sequence;
  const next = [...logLines.value, formatLogLine(item)];
  if (next.length > LOG_LINE_LIMIT) {
    const overflow = next.length - LOG_LINE_LIMIT;
    next.splice(0, overflow);
    discardedLines.value += overflow;
  }
  logLines.value = next;
  if (session.value) {
    session.value.event_count = Math.max(
      session.value.event_count,
      item.sequence,
    );
  }
  if (autoScroll) scrollToLatest();
};

const closeLive = () => {
  source?.close();
  source = null;
};

const openLive = () => {
  closeLive();
  if (!session.value || !active.value) return;
  source = new EventSource(
    DeepMonitorAPI.liveUrl(session.value.id, lastSequence.value),
    { withCredentials: true },
  );
  source.addEventListener("traffic", (raw) => {
    try {
      appendEvent(
        JSON.parse(
          (raw as MessageEvent<string>).data,
        ) as DeepMonitorEventSummary,
      );
    } catch (error) {
      console.warn("invalid deep monitor event", error);
    }
  });
};

const resetLogStream = () => {
  closeLive();
  logLines.value = [];
  lastSequence.value = 0;
  discardedLines.value = 0;
};

const loadRecentEvents = async (signal?: AbortSignal) => {
  const current = session.value;
  if (!current) return;
  loading.value = true;
  try {
    const approximateFirstSequence = Math.max(
      0,
      current.event_count - LOG_LINE_LIMIT,
    );
    discardedLines.value = approximateFirstSequence;
    let cursor = String(approximateFirstSequence);
    let hasMore = true;
    while (hasMore && logLines.value.length < LOG_LINE_LIMIT) {
      const result = await DeepMonitorAPI.events(
        current.id,
        {
          cursor,
          limit: 200,
        },
        signal,
      );
      for (const item of result.items) appendEvent(item, false);
      cursor = result.next_cursor;
      hasMore = result.has_more && result.items.length > 0;
    }
    scrollToLatest();
  } catch (error) {
    toast.error(errorMessage(error));
  } finally {
    loading.value = false;
  }
};

const refreshSession = async (
  forceReload = false,
  silent = false,
  signal?: AbortSignal,
) => {
  if (mutating.value && !forceReload) return;
  try {
    const items = (await DeepMonitorAPI.list(signal)).filter(
      (item) => normalizeHost(item.host) === normalizeHost(host.value),
    );
    const next =
      items.find((item) => item.state === "active") ?? items[0] ?? null;
    const changed = forceReload || next?.id !== session.value?.id;
    session.value = next;
    if (changed) {
      resetLogStream();
      await loadRecentEvents(signal);
      openLive();
    } else if (!active.value) {
      closeLive();
    } else if (!source) {
      openLive();
    }
  } catch (error) {
    if (!silent) toast.error(errorMessage(error));
  }
};

const sessionPoller = createVisibilityPoller({
  intervalMs: 5_000,
  task: (signal) => refreshSession(false, true, signal),
});

const start = async () => {
  if (!host.value || active.value) return;
  mutating.value = true;
  try {
    session.value = await DeepMonitorAPI.start({
      host: host.value,
      duration_seconds: DEFAULT_DURATION_SECONDS,
    });
    resetLogStream();
    openLive();
    toast.success(t("admin.deepMonitor.started"));
  } catch (error) {
    toast.error(errorMessage(error));
  } finally {
    mutating.value = false;
  }
};

const stop = async () => {
  if (!session.value || !active.value) return;
  mutating.value = true;
  try {
    session.value = await DeepMonitorAPI.stop(session.value.id);
    closeLive();
    toast.success(t("admin.deepMonitor.stopped"));
  } catch (error) {
    toast.error(errorMessage(error));
  } finally {
    mutating.value = false;
  }
};

const clear = async () => {
  if (active.value) return;
  mutating.value = true;
  try {
    const sessions = (await DeepMonitorAPI.list()).filter(
      (item) =>
        item.state !== "active" &&
        normalizeHost(item.host) === normalizeHost(host.value),
    );
    for (const item of sessions) await DeepMonitorAPI.delete(item.id);
    session.value = null;
    resetLogStream();
    toast.success(t("admin.deepMonitor.cleared"));
  } catch (error) {
    toast.error(errorMessage(error));
  } finally {
    mutating.value = false;
  }
};

watch(host, () => sessionPoller.sync());

onMounted(async () => {
  clock = window.setInterval(() => (now.value = Date.now()), 1000);
  await refreshSession(true);
  sessionPoller.start();
});

onUnmounted(() => {
  closeLive();
  if (clock) window.clearInterval(clock);
  sessionPoller.stop();
});
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/subdomains">{{
            t("admin.nav.subdomainMapping")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{ t("admin.deepMonitor.title") }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <div class="flex flex-wrap items-start justify-between gap-4">
      <div class="min-w-0">
        <h1 class="text-2xl font-semibold tracking-tight">
          {{ t("admin.deepMonitor.title") }}
        </h1>
        <p class="mt-1 break-all font-mono text-sm text-muted-foreground">
          {{ host }}
        </p>
      </div>
      <div class="flex flex-wrap items-center justify-end gap-2">
        <Button v-if="!active" :disabled="mutating || !host" @click="start">
          <Activity class="mr-2 h-4 w-4" />{{ t("admin.deepMonitor.start") }}
        </Button>
        <Button v-else variant="destructive" :disabled="mutating" @click="stop">
          <Square class="mr-2 h-4 w-4" />{{ t("admin.deepMonitor.stop") }}
        </Button>
        <Button v-if="session" variant="outline" as-child>
          <a
            :href="DeepMonitorAPI.archiveUrl(session.id)"
            :download="`deep-monitor-${session.id}.zip`"
          >
            <Download class="mr-2 h-4 w-4" />{{
              t("admin.deepMonitor.downloadLogs")
            }}
          </a>
        </Button>
        <Button v-else variant="outline" disabled>
          <Download class="mr-2 h-4 w-4" />{{
            t("admin.deepMonitor.downloadLogs")
          }}
        </Button>
        <Button
          variant="outline"
          :disabled="mutating || active || !session"
          @click="clear"
        >
          <Eraser class="mr-2 h-4 w-4" />{{ t("admin.deepMonitor.clearLogs") }}
        </Button>
      </div>
    </div>

    <div
      class="flex flex-wrap items-center gap-x-3 gap-y-1 border-y py-3 text-sm text-muted-foreground"
    >
      <span class="inline-flex items-center gap-2 font-medium text-foreground">
        <span class="relative flex h-2 w-2">
          <span
            v-if="active"
            class="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-500 opacity-70"
          />
          <span
            class="relative inline-flex h-2 w-2 rounded-full"
            :class="active ? 'bg-emerald-500' : 'bg-muted-foreground/50'"
          />
        </span>
        {{ stateLabel }}
      </span>
      <template v-if="session">
        <span v-if="remaining">{{ remaining }}</span>
        <span>{{ formatBytes(session.bytes_stored) }}</span>
        <span>{{
          t("admin.deepMonitor.eventCount", { count: session.event_count })
        }}</span>
      </template>
      <span class="ml-auto">{{
        t("admin.deepMonitor.captureDefaults", {
          minutes: DEFAULT_DURATION_SECONDS / 60,
        })
      }}</span>
    </div>

    <section
      class="overflow-hidden rounded-lg border bg-zinc-950 text-zinc-100"
    >
      <header
        class="flex flex-wrap items-center gap-2 border-b border-white/10 px-4 py-2 font-mono text-xs text-zinc-400"
      >
        <span>{{ t("admin.deepMonitor.liveLog") }}</span>
        <span>·</span>
        <span>{{
          t("admin.deepMonitor.logLineLimit", { count: LOG_LINE_LIMIT })
        }}</span>
        <span v-if="discardedLines">·</span>
        <span v-if="discardedLines">{{
          t("admin.deepMonitor.olderLinesDiscarded", {
            count: discardedLines,
          })
        }}</span>
      </header>
      <pre
        ref="logViewport"
        class="h-[min(65vh,44rem)] min-h-80 overflow-auto p-4 font-mono text-xs leading-5"
        :class="{ 'flex items-center justify-center text-zinc-500': !logText }"
        >{{
          logText ||
          (loading
            ? t("common.loading")
            : t("admin.deepMonitor.waitingForTraffic"))
        }}</pre>
    </section>
  </div>
</template>
