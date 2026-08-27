<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  AlertTriangle,
  ArrowLeft,
  Bell,
  Loader2,
  RefreshCw,
  Route as RouteIcon,
  ShieldCheck,
  Webhook,
} from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import TraceIdLink from "@/components/TraceIdLink.vue";
import { TraceAPI, type TraceLookupPayload } from "@/lib/api/traces";
import { isTraceId, normalizeTraceId } from "@/lib/trace-id";

type JsonRecord = Record<string, unknown>;
type TimelineKind = "request" | "waf" | "event" | "trigger" | "delivery";
interface TimelineItem {
  id: string;
  kind: TimelineKind;
  time: string;
  title: string;
  detail: string;
}

const route = useRoute();
const router = useRouter();
const { t, locale } = useI18n();
const loading = ref(false);
const loadError = ref(false);
const invalidTraceId = ref(false);
const payload = ref<TraceLookupPayload | null>(null);
const query = ref("");
let controller: AbortController | null = null;

const traceId = computed(() => String(route.params.trace_id || "").trim());
const request = computed(
  () => (payload.value?.request || null) as JsonRecord | null,
);
const wafEvent = computed(
  () => (payload.value?.waf_event || null) as JsonRecord | null,
);
const triggers = computed(
  () => (payload.value?.notification_triggers || []) as JsonRecord[],
);
const deliveries = computed(
  () => (payload.value?.notification_deliveries || []) as JsonRecord[],
);
const unavailableSources = computed(() =>
  payload.value
    ? Object.entries(payload.value.sources)
        .filter(([, status]) => status === "unavailable")
        .map(([source]) => t(`admin.trace.sources.${source}`))
    : [],
);

const stringValue = (record: JsonRecord, ...keys: string[]) => {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value) return value;
    if (typeof value === "number") return String(value);
  }
  return "";
};

const timeline = computed<TimelineItem[]>(() => {
  const items: TimelineItem[] = [];
  if (request.value) {
    items.push({
      id: "request",
      kind: "request",
      time: stringValue(request.value, "time"),
      title: t("admin.trace.timeline.request"),
      detail:
        `${stringValue(request.value, "method")} ${stringValue(request.value, "request_uri", "path")}`.trim(),
    });
  }
  if (wafEvent.value) {
    items.push({
      id: "waf",
      kind: "waf",
      time: stringValue(wafEvent.value, "time"),
      title: t("admin.trace.timeline.waf"),
      detail: stringValue(wafEvent.value, "action", "mode"),
    });
  }
  for (const event of payload.value?.system_events || []) {
    items.push({
      id: `event-${event.id}`,
      kind: "event",
      time: event.happened_at,
      title: t("admin.trace.timeline.event"),
      detail: event.type,
    });
  }
  for (const trigger of triggers.value) {
    items.push({
      id: `trigger-${stringValue(trigger, "id")}`,
      kind: "trigger",
      time: stringValue(trigger, "created_at"),
      title: t("admin.trace.timeline.trigger"),
      detail: stringValue(trigger, "rule_id", "id"),
    });
  }
  for (const delivery of deliveries.value) {
    items.push({
      id: `delivery-${stringValue(delivery, "id")}`,
      kind: "delivery",
      time: stringValue(delivery, "sent_at", "triggered_at"),
      title: t("admin.trace.timeline.delivery"),
      detail: stringValue(delivery, "status", "provider_type"),
    });
  }
  return items.sort((left, right) => {
    const leftTime = Date.parse(left.time);
    const rightTime = Date.parse(right.time);
    if (Number.isNaN(leftTime) && Number.isNaN(rightTime)) {
      return left.id.localeCompare(right.id);
    }
    if (Number.isNaN(leftTime)) return 1;
    if (Number.isNaN(rightTime)) return -1;
    return leftTime - rightTime;
  });
});

const iconFor = (kind: TimelineKind) =>
  ({
    request: RouteIcon,
    waf: ShieldCheck,
    event: Webhook,
    trigger: Bell,
    delivery: Bell,
  })[kind];

const formatTime = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value || "-"
    : new Intl.DateTimeFormat(String(locale.value), {
        dateStyle: "medium",
        timeStyle: "medium",
      }).format(date);
};

const pretty = (value: unknown) => JSON.stringify(value, null, 2);
const statusLabel = (status: string) => t(`admin.trace.status.${status}`);

const load = async () => {
  controller?.abort();
  const currentTraceId = normalizeTraceId(traceId.value);
  query.value = currentTraceId;
  invalidTraceId.value = !isTraceId(currentTraceId);
  if (invalidTraceId.value) {
    controller = null;
    loading.value = false;
    loadError.value = false;
    payload.value = null;
    return;
  }

  const requestController = new AbortController();
  controller = requestController;
  loading.value = true;
  loadError.value = false;
  payload.value = null;
  try {
    const response = await TraceAPI.get(
      currentTraceId,
      requestController.signal,
    );
    if (controller !== requestController || requestController.signal.aborted) {
      return;
    }
    payload.value = response.data;
  } catch (_error) {
    if (controller !== requestController || requestController.signal.aborted) {
      return;
    }
    loadError.value = true;
  } finally {
    if (controller === requestController) {
      controller = null;
      loading.value = false;
    }
  }
};

const search = () => {
  const value = normalizeTraceId(query.value);
  invalidTraceId.value = !isTraceId(value);
  if (invalidTraceId.value) return;
  if (value !== traceId.value) {
    void router.push(`/traces/${encodeURIComponent(value)}`);
    return;
  }
  void load();
};

watch(traceId, load, { immediate: true });
onBeforeUnmount(() => controller?.abort());
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface min-h-full space-y-4"
  >
    <div
      class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between"
    >
      <div class="min-w-0 space-y-2">
        <Button variant="ghost" class="-ml-3 h-8" @click="router.back()">
          <ArrowLeft class="mr-2 h-4 w-4" />
          {{ t("admin.trace.back") }}
        </Button>
        <div>
          <h1 class="text-xl font-semibold tracking-tight">
            {{ t("admin.trace.title") }}
          </h1>
          <p class="mt-1 text-sm text-muted-foreground">
            {{ t("admin.trace.description") }}
          </p>
        </div>
        <TraceIdLink v-if="isTraceId(traceId)" :trace-id="traceId" />
      </div>
      <form class="flex w-full gap-2 lg:max-w-xl" @submit.prevent="search">
        <Input
          v-model="query"
          class="min-w-0 font-mono"
          spellcheck="false"
          :aria-label="t('admin.trace.lookup')"
          :aria-invalid="invalidTraceId"
          @input="invalidTraceId = false"
        />
        <Button type="submit" variant="outline" :disabled="loading">
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': loading }"
          />
          {{ t("admin.trace.search") }}
        </Button>
      </form>
    </div>

    <Alert v-if="invalidTraceId" variant="destructive">
      <AlertTriangle />
      <AlertTitle>{{ t("admin.trace.invalid") }}</AlertTitle>
      <AlertDescription>{{
        t("admin.trace.inputPlaceholder")
      }}</AlertDescription>
    </Alert>

    <div v-else-if="loading" class="flex min-h-52 items-center justify-center">
      <Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
    </div>

    <Alert v-else-if="loadError" variant="destructive">
      <AlertTriangle />
      <AlertTitle>{{ t("admin.trace.loadFailed") }}</AlertTitle>
      <AlertDescription>{{
        t("admin.trace.loadFailedDescription")
      }}</AlertDescription>
    </Alert>

    <template v-else-if="payload">
      <Alert v-if="unavailableSources.length">
        <AlertTriangle />
        <AlertTitle>{{ t("admin.trace.partialTitle") }}</AlertTitle>
        <AlertDescription>
          {{
            t("admin.trace.partialDescription", {
              sources: unavailableSources.join(", "),
            })
          }}
        </AlertDescription>
      </Alert>

      <Alert v-if="!payload.found">
        <AlertTriangle />
        <AlertTitle>{{ t("admin.trace.notFound") }}</AlertTitle>
        <AlertDescription>{{ t("admin.trace.missing") }}</AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{{ t("admin.trace.timeline.title") }}</CardTitle>
        </CardHeader>
        <CardContent>
          <p v-if="timeline.length === 0" class="text-sm text-muted-foreground">
            {{ t("admin.trace.timeline.empty") }}
          </p>
          <ol v-else class="relative ml-4 border-l">
            <li
              v-for="item in timeline"
              :key="item.id"
              class="relative pb-6 pl-7 last:pb-0"
            >
              <span
                class="absolute -left-[17px] top-0 flex h-8 w-8 items-center justify-center rounded-full border bg-background"
              >
                <component :is="iconFor(item.kind)" class="h-4 w-4" />
              </span>
              <div
                class="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between"
              >
                <span class="font-medium">{{ item.title }}</span>
                <time class="text-xs text-muted-foreground">{{
                  formatTime(item.time)
                }}</time>
              </div>
              <p class="mt-1 break-all font-mono text-xs text-muted-foreground">
                {{ item.detail || "-" }}
              </p>
            </li>
          </ol>
        </CardContent>
      </Card>

      <div class="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader class="flex-row items-center justify-between">
            <CardTitle>{{ t("admin.trace.sections.request") }}</CardTitle>
            <Badge variant="outline">{{
              statusLabel(payload.sources.gateway_logs)
            }}</Badge>
          </CardHeader>
          <CardContent>
            <pre
              v-if="request"
              class="max-h-[420px] overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted p-4 text-xs"
              >{{ pretty(request) }}</pre>
            <p v-else class="text-sm text-muted-foreground">
              {{ t("admin.trace.missing") }}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader class="flex-row items-center justify-between">
            <CardTitle>{{ t("admin.trace.sections.waf") }}</CardTitle>
            <Badge variant="outline">{{
              statusLabel(payload.sources.waf_logs)
            }}</Badge>
          </CardHeader>
          <CardContent>
            <pre
              v-if="wafEvent"
              class="max-h-[420px] overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted p-4 text-xs"
              >{{ pretty(wafEvent) }}</pre>
            <p v-else class="text-sm text-muted-foreground">
              {{ t("admin.trace.missing") }}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader class="flex-row items-center justify-between">
            <CardTitle>{{ t("admin.trace.sections.events") }}</CardTitle>
            <Badge variant="outline">{{
              statusLabel(payload.sources.system_events)
            }}</Badge>
          </CardHeader>
          <CardContent class="space-y-3">
            <p
              v-if="payload.system_events.length === 0"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.trace.missing") }}
            </p>
            <pre
              v-for="event in payload.system_events"
              v-else
              :key="event.id"
              class="max-h-[320px] overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted p-4 text-xs"
              >{{ pretty(event) }}</pre>
          </CardContent>
        </Card>

        <Card>
          <CardHeader class="flex-row items-center justify-between">
            <CardTitle>{{ t("admin.trace.sections.notifications") }}</CardTitle>
            <Badge variant="outline">{{
              statusLabel(payload.sources.notifications)
            }}</Badge>
          </CardHeader>
          <CardContent class="space-y-3">
            <p
              v-if="triggers.length === 0 && deliveries.length === 0"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.trace.missing") }}
            </p>
            <div v-if="triggers.length" class="space-y-2">
              <h3 class="text-sm font-medium">
                {{ t("admin.trace.sections.triggers") }}
              </h3>
              <pre
                v-for="trigger in triggers"
                :key="String(trigger.id)"
                class="max-h-[300px] overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted p-4 text-xs"
                >{{ pretty(trigger) }}</pre>
            </div>
            <div v-if="deliveries.length" class="space-y-2">
              <h3 class="text-sm font-medium">
                {{ t("admin.trace.sections.deliveries") }}
              </h3>
              <pre
                v-for="delivery in deliveries"
                :key="String(delivery.id)"
                class="max-h-[300px] overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted p-4 text-xs"
                >{{ pretty(delivery) }}</pre>
            </div>
          </CardContent>
        </Card>
      </div>
    </template>
  </div>
</template>
