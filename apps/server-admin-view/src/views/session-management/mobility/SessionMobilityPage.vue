<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/sessions?tab=sessions">会话管理</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>IP 漂移轨迹</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 bg-background shadow-none">
      <CardHeader class="gap-0">
        <div class="min-w-0 space-y-3">
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
              <CardTitle class="break-words text-xl font-semibold tracking-[0.02em] sm:text-[1.65rem]">IP 漂移轨迹</CardTitle>
              <Badge v-if="session" variant="secondary" class="rounded-full border border-border/40 bg-muted/30 px-2.5 py-0.5 text-muted-foreground shadow-none">
                {{ session.method }}
              </Badge>
            </div>

            <Button
              variant="outline"
              size="icon"
              class="mt-0.5 h-9 w-9 shrink-0 rounded-full border-border/40 bg-background text-muted-foreground transition-colors hover:bg-muted/30 hover:text-foreground"
              :aria-label="sortToggleLabel"
              :title="sortToggleLabel"
              @click="toggleSortOrder"
            >
              <ArrowUpDown
                class="h-4 w-4 transition-transform duration-200"
                :class="sortOrder === 'desc' ? 'rotate-180' : ''"
              />
              <span class="sr-only">{{ sortToggleLabel }}</span>
            </Button>
          </div>

          <CardDescription class="max-w-2xl break-all text-sm leading-7 text-muted-foreground/90">
            {{ headerDescription }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent class="space-y-7 pt-0 px-5">
        <div v-if="isLoading" class="flex items-center justify-center py-16 text-sm text-muted-foreground">
          <span class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"></span>
          正在加载轨迹...
        </div>

        <div v-else-if="loadError" class="rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-5">
          <div class="text-sm font-medium text-destructive">轨迹加载失败</div>
          <div class="mt-1 text-sm text-muted-foreground">{{ loadError }}</div>
        </div>

        <template v-else-if="session">
          <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-[minmax(0,1.2fr)_repeat(3,minmax(0,1fr))]">
            <div class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4 sm:col-span-2 xl:col-span-1">
              <div class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90">当前会话</div>
              <div class="mt-3 break-words text-sm font-medium text-foreground">{{ session.credentialName }}</div>
              <div class="mt-3 break-all font-mono text-sm text-foreground">{{ session.ip }}</div>
              <div class="mt-1 break-words text-xs leading-6 text-muted-foreground">{{ session.ipLocation || '暂无归属信息' }}</div>
            </div>

            <div class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4">
              <div class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90">恢复次数</div>
              <div class="mt-3 text-xl font-semibold text-foreground">{{ mobilitySummary?.driftCount ?? 0 }}</div>
              <div class="mt-1 break-words text-xs leading-6 text-muted-foreground">{{ driftCountDescription }}</div>
            </div>

            <div class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4">
              <div class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90">轨迹跨度</div>
              <div class="mt-3 break-words text-sm font-semibold text-foreground">{{ timelineSpanLabel }}</div>
              <div class="mt-1 break-words text-xs leading-6 text-muted-foreground">{{ timelineSpanDescription }}</div>
            </div>

            <div class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4">
              <div class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90">最近变化</div>
              <div class="mt-3 break-words text-sm font-semibold text-foreground">
                <HumanFriendlyTime
                  v-if="lastEventTimeValue"
                  :value="lastEventTimeValue"
                />
                <template v-else>{{ lastEventTimeLabel }}</template>
              </div>
              <div class="mt-1 break-words text-xs leading-6 text-muted-foreground">{{ lastEventSourceLabel }}</div>
            </div>
          </div>

          <div v-if="timelineEntries.length > 0" class="relative border-t border-border/35 pt-6 sm:pt-8">
              <div class="absolute bottom-4 left-6 top-8 w-px bg-border/70 sm:left-8 sm:top-10" />
              <div class="space-y-0">
                <div
                  v-for="entry in timelineEntries"
                  :key="entry.id"
                  class="relative border-b border-border/40 py-5 pl-11 first:pt-0 last:border-b-0 last:pb-0 sm:py-6 sm:pl-14"
                >
                  <div class="pointer-events-none absolute left-6 top-5 -translate-x-1/2 sm:left-8">
                    <LiveStatusBadge
                      v-if="entry.id === latestEntryId"
                      active
                      :pulse="false"
                      active-label="最新状态"
                      size="sm"
                      class="block"
                    />
                    <div
                      v-else
                      class="h-3 w-3 rounded-full border-2 border-background bg-foreground/90"
                    />
                  </div>
                  <div class="space-y-4">
                    <div class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
                      <div class="min-w-0 space-y-1">
                        <div class="flex flex-wrap items-center gap-2">
                          <Badge
                            :variant="entry.event.kind === 'login' ? 'secondary' : 'outline'"
                            class="rounded-full border-border/40 bg-background/80 px-2.5 py-0.5 text-[12px] font-medium shadow-none"
                          >
                            {{ entry.event.kind === 'login' ? '登录建立' : 'IP 恢复' }}
                          </Badge>
                          <span class="break-words text-sm font-medium leading-6 text-foreground">{{ entry.title }}</span>
                        </div>
                        <div class="break-words text-xs leading-6 text-muted-foreground">{{ entry.subtitle }}</div>
                      </div>
                      <div class="text-left text-xs leading-6 text-muted-foreground sm:shrink-0 sm:pl-6 sm:text-right">
                        <div>
                          <HumanFriendlyTime :value="entry.event.happenedAt" />
                        </div>
                        <div v-if="entry.gapLabel" class="mt-1">{{ entry.gapLabel }}</div>
                      </div>
                    </div>

                    <div
                      v-if="entry.event.kind === 'login'"
                      class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-foreground"
                    >
                      <span class="break-all font-mono">{{ entry.event.toIp }}</span>
                      <span class="text-muted-foreground/70">·</span>
                      <span class="break-words text-muted-foreground">{{ entry.event.toIpLocation || '暂无归属信息' }}</span>
                    </div>

                    <div v-else class="mt-1 grid gap-3 text-sm sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] sm:items-center">
                      <div class="min-w-0">
                        <div class="text-[11px] uppercase tracking-[0.16em] text-muted-foreground/85">漂移前</div>
                        <div class="mt-2 break-all font-mono text-foreground">{{ entry.event.fromIp }}</div>
                        <div class="mt-1 break-words text-xs leading-6 text-muted-foreground">{{ entry.event.fromIpLocation || '暂无归属信息' }}</div>
                      </div>
                      <div class="flex justify-center text-muted-foreground/70">
                        <ArrowRight class="h-4 w-4 rotate-90 sm:rotate-0" />
                      </div>
                      <div class="min-w-0">
                        <div class="text-[11px] uppercase tracking-[0.16em] text-muted-foreground/85">漂移后</div>
                        <div class="mt-2 break-all font-mono text-foreground">{{ entry.event.toIp }}</div>
                        <div class="mt-1 break-words text-xs leading-6 text-muted-foreground">{{ entry.event.toIpLocation || '暂无归属信息' }}</div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
          </div>

          <div v-else class="border-t border-border/35 px-4 py-12 text-center text-sm text-muted-foreground">
            当前会话没有可展示的轨迹记录。
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import { ArrowRight, ArrowUpDown } from 'lucide-vue-next';
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from '@/components/ui/breadcrumb';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import LiveStatusBadge from '@/components/LiveStatusBadge.vue';
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue';
import { SessionAPI } from '../../../lib/api';
import type { SessionMobilityDetails, SessionMobilityEvent, SessionRecord } from '../../../types';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';

type SortOrder = 'asc' | 'desc';

type TimelineEntry = {
  id: string;
  event: SessionMobilityEvent;
  title: string;
  subtitle: string;
  gapLabel: string | null;
  happenedAtMs: number;
};

const route = useRoute();

const session = ref<SessionRecord | null>(null);
const mobility = ref<SessionMobilityDetails | null>(null);
const loadError = ref('');
const sortOrder = ref<SortOrder>('desc');

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    loadError.value = extractErrorMessage(error, '加载轨迹失败');
  },
});

const sessionId = computed(() => String(route.params.id || ''));
const mobilitySummary = computed(() => mobility.value?.summary ?? session.value?.mobility ?? null);
const sortToggleLabel = computed(() => {
  return sortOrder.value === 'desc' ? '当前为倒序，点击切换为正序' : '当前为正序，点击切换为倒序';
});

const headerDescription = computed(() => {
  if (!session.value) return '查看会话在使用期间的 IP 变化与恢复过程。';
  return `${session.value.credentialName} · 会话 ${middleEllipsis(session.value.id, 20)}`;
});

const driftCountDescription = computed(() => {
  const count = mobilitySummary.value?.driftCount ?? 0;
  if (count === 0) return '会话期间未发生 IP 变化';
  if (count === 1) return '已完成 1 次自动恢复';
  return `已完成 ${count} 次自动恢复`;
});

const chronologicalEntries = computed<TimelineEntry[]>(() => {
  const events = [...(mobility.value?.events ?? [])].sort((a, b) => {
    return (Date.parse(a.happenedAt) || 0) - (Date.parse(b.happenedAt) || 0);
  });

  return events.map((event, index) => {
    const previous = index > 0 ? events[index - 1] : null;
    const happenedAtMs = Date.parse(event.happenedAt) || 0;
    const previousMs = previous ? Date.parse(previous.happenedAt) || 0 : 0;
    const gapMs = previous && happenedAtMs > previousMs ? happenedAtMs - previousMs : 0;

    if (event.kind === 'login') {
      return {
        id: `login-${event.happenedAt}-${index}`,
        event,
        title: '建立登录会话',
        subtitle: '记录首次通过认证时的来源 IP',
        gapLabel: null,
        happenedAtMs,
      };
    }

    return {
      id: `drift-${event.happenedAt}-${index}`,
      event,
      title: formatSourceLabel(event.source),
      subtitle: '检测到来源 IP 变化后完成会话续接',
      gapLabel: gapMs > 0 ? `距上一节点 ${formatDuration(gapMs)}` : null,
      happenedAtMs,
    };
  });
});

const timelineEntries = computed(() => {
  const entries = [...chronologicalEntries.value];
  return sortOrder.value === 'desc' ? entries.reverse() : entries;
});
const latestEntryId = computed(() => {
  return chronologicalEntries.value[chronologicalEntries.value.length - 1]?.id ?? '';
});

const timelineSpanMs = computed(() => {
  const entries = chronologicalEntries.value;
  if (entries.length < 2) return 0;
  const first = entries[0]?.happenedAtMs ?? 0;
  const last = entries[entries.length - 1]?.happenedAtMs ?? 0;
  return Math.max(0, last - first);
});

const timelineSpanLabel = computed(() => {
  if (timelineSpanMs.value <= 0) return '暂无跨度';
  return formatDuration(timelineSpanMs.value);
});

const timelineSpanDescription = computed(() => {
  const count = chronologicalEntries.value.length;
  if (count <= 1) return '当前仅有登录起点记录';
  return '从首次登录到最近一次变化';
});

const lastEvent = computed(() => {
  const entries = chronologicalEntries.value;
  return entries[entries.length - 1] ?? null;
});

const lastEventTimeLabel = computed(() => {
  if (!lastEvent.value) return '暂无记录';
  if (lastEvent.value.event.kind === 'login') return '仅有登录记录';
  return '';
});

const lastEventTimeValue = computed(() => {
  if (!lastEvent.value || lastEvent.value.event.kind === 'login') return null;
  return lastEvent.value.event.happenedAt;
});

const lastEventSourceLabel = computed(() => {
  if (!lastEvent.value) return '尚无变化来源';
  if (lastEvent.value.event.kind === 'login') return '尚未发生 IP 变化';
  return `来源：${formatSourceLabel(lastEvent.value.event.source)}`;
});

function middleEllipsis(text: string, max = 16) {
  if (!text) return '';
  if (text.length <= max) return text;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${text.slice(0, head)}……${text.slice(text.length - tail)}`;
}

function formatSourceLabel(source: 'login' | 'proxy-session' | 'fnos-token') {
  if (source === 'login') return '登录建立';
  if (source === 'fnos-token') return '飞牛指纹续接恢复';
  return '会话漂移续接恢复';
}

function formatDuration(ms: number) {
  const totalMinutes = Math.max(0, Math.floor(ms / 60000));
  const days = Math.floor(totalMinutes / (60 * 24));
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
  const minutes = totalMinutes % 60;

  if (days > 0) {
    return hours > 0 ? `${days}天 ${hours}小时` : `${days}天`;
  }
  if (hours > 0) {
    return minutes > 0 ? `${hours}小时 ${minutes}分钟` : `${hours}小时`;
  }
  if (minutes > 0) {
    return `${minutes}分钟`;
  }
  return '不足 1 分钟';
}

async function fetchData() {
  if (!sessionId.value) return;
  loadError.value = '';
  session.value = null;
  mobility.value = null;
  await runLoad(async () => {
    const [sessionData, mobilityData] = await Promise.all([
      SessionAPI.get(sessionId.value),
      SessionAPI.getMobility(sessionId.value),
    ]);
    session.value = sessionData;
    mobility.value = mobilityData;
  });
}

function toggleSortOrder() {
  sortOrder.value = sortOrder.value === 'desc' ? 'asc' : 'desc';
}

watch(
  sessionId,
  () => {
    void fetchData();
  },
  { immediate: true },
);
</script>
