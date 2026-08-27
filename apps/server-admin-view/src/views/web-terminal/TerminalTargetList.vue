<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  LoaderCircle,
  Pencil,
  Plus,
  Server,
  SquareTerminal,
  Trash2,
} from "lucide-vue-next";
import type {
  TerminalSessionRecord,
  TerminalTargetRecord,
} from "@/lib/api/terminal";

const props = defineProps<{
  collapsed?: boolean;
  drawer?: boolean;
  loading: boolean;
  selectedSessionId: string;
  selectedTargetId: string;
  sessions: TerminalSessionRecord[];
  targets: TerminalTargetRecord[];
}>();

const emit = defineEmits<{
  add: [];
  delete: [target: TerminalTargetRecord];
  edit: [target: TerminalTargetRecord];
  selectSession: [sessionId: string];
  select: [targetId: string];
}>();

const { t } = useI18n();
const activePhases = new Set([
  "creating",
  "resolving",
  "connecting",
  "verifyingHostKey",
  "authenticating",
  "openingChannel",
  "requestingPty",
  "running",
  "closing",
]);
const counts = computed(() => {
  const result = new Map<string, number>();
  for (const session of props.sessions) {
    if (!activePhases.has(session.phase)) continue;
    result.set(session.targetId, (result.get(session.targetId) ?? 0) + 1);
  }
  return result;
});
const activeCount = (targetId: string) => counts.value.get(targetId) ?? 0;
const sessionsForTarget = (targetId: string) =>
  props.sessions.filter((session) => session.targetId === targetId);
const sessionStatus = (session: TerminalSessionRecord) => {
  const label = t(`admin.webTerminal.sessionPhase.${session.phase}`);
  if (session.phase === "running") {
    return {
      label,
      tone: "bg-emerald-500",
    };
  }
  if (session.phase === "failed" || session.phase === "lost") {
    return {
      label,
      tone: "bg-destructive",
    };
  }
  if (session.phase === "closed" || session.phase === "exited") {
    return {
      label,
      tone: "bg-muted-foreground/50",
    };
  }
  return {
    label,
    tone: "bg-amber-500",
  };
};
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      :class="[
        'flex shrink-0 items-center border-b border-border/70 p-3',
        collapsed ? 'justify-center' : 'justify-between',
        drawer ? 'pr-14' : '',
      ]"
    >
      <div v-if="!collapsed" class="min-w-0">
        <p class="truncate text-sm font-semibold">
          {{ t("admin.webTerminal.targets", "SSH targets") }}
        </p>
        <p class="text-[11px] text-muted-foreground">
          {{ targets.length }}
          {{ t("admin.webTerminal.targetCountSuffix", "configured") }}
        </p>
      </div>
      <Button
        size="icon-sm"
        variant="outline"
        :aria-label="t('admin.webTerminal.addTarget', 'Add SSH target')"
        :title="t('admin.webTerminal.addTarget', 'Add SSH target')"
        @click="emit('add')"
      >
        <Plus class="h-4 w-4" />
      </Button>
    </div>

    <div class="min-h-0 flex-1 space-y-1 overflow-y-auto p-2">
      <div
        v-if="loading"
        class="flex items-center justify-center gap-2 px-2 py-8 text-xs text-muted-foreground"
      >
        <LoaderCircle class="h-4 w-4 animate-spin" />
        <span v-if="!collapsed">{{ t("common.loading") }}</span>
      </div>

      <div
        v-else-if="targets.length === 0"
        :class="[
          'rounded-xl border border-dashed border-border/80 text-center text-muted-foreground',
          collapsed ? 'p-2' : 'px-3 py-8',
        ]"
      >
        <Server class="mx-auto h-5 w-5" />
        <template v-if="!collapsed">
          <p class="mt-2 text-xs font-medium text-foreground">
            {{ t("admin.webTerminal.noTargets", "No SSH targets") }}
          </p>
          <p class="mt-1 text-[11px] leading-4">
            {{
              t(
                "admin.webTerminal.noTargetsDescription",
                "Add a server before opening a terminal session.",
              )
            }}
          </p>
        </template>
      </div>

      <div
        v-for="target in targets"
        :key="target.id"
        :class="[
          'group relative rounded-xl border transition-colors',
          selectedTargetId === target.id
            ? 'border-primary/35 bg-primary/7'
            : 'border-transparent hover:border-border/70 hover:bg-muted/40',
        ]"
      >
        <button
          type="button"
          :class="[
            'flex w-full items-center text-left outline-none focus-visible:ring-2 focus-visible:ring-ring',
            collapsed ? 'justify-center p-2.5' : 'gap-2.5 px-3 py-2.5 pr-20',
          ]"
          :aria-label="target.name"
          :title="
            collapsed
              ? `${target.name} — ${target.username}@${target.host}`
              : undefined
          "
          @click="emit('select', target.id)"
        >
          <span class="relative shrink-0">
            <Server class="h-4 w-4" />
            <span
              v-if="activeCount(target.id)"
              class="absolute -right-1 -top-1 h-2 w-2 rounded-full border border-background bg-emerald-500"
            />
          </span>
          <span v-if="!collapsed" class="min-w-0 flex-1">
            <span class="flex items-center gap-1.5">
              <span class="truncate text-xs font-medium">{{
                target.name
              }}</span>
              <Badge
                v-if="activeCount(target.id)"
                variant="secondary"
                class="h-4 px-1 text-[9px] tabular-nums"
              >
                {{ activeCount(target.id) }}
              </Badge>
            </span>
            <span
              class="mt-0.5 block truncate text-[10px] text-muted-foreground"
            >
              {{ target.username }}@{{ target.host }}:{{ target.port }}
            </span>
          </span>
        </button>

        <div
          v-if="!collapsed"
          class="absolute right-1.5 top-1.5 flex opacity-70 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
        >
          <Button
            size="icon-sm"
            variant="ghost"
            :aria-label="t('common.edit')"
            :title="t('common.edit')"
            @click.stop="emit('edit', target)"
          >
            <Pencil class="h-3.5 w-3.5" />
          </Button>
          <ConfirmDangerPopover
            :title="
              t('admin.webTerminal.deleteTargetTitle', 'Delete SSH target?')
            "
            :description="
              activeCount(target.id)
                ? t('admin.webTerminal.deleteTargetActiveDescription', {
                    count: activeCount(target.id),
                  })
                : t(
                    'admin.webTerminal.deleteTargetDescription',
                    'The saved target and its encrypted credential will be removed.',
                  )
            "
            :confirm-text="t('common.delete')"
            :on-confirm="() => emit('delete', target)"
          >
            <template #trigger>
              <Button
                size="icon-sm"
                variant="ghost"
                class="text-destructive hover:text-destructive"
                :aria-label="t('common.delete')"
                :title="t('common.delete')"
              >
                <Trash2 class="h-3.5 w-3.5" />
              </Button>
            </template>
          </ConfirmDangerPopover>
        </div>

        <div
          v-if="
            !collapsed &&
            selectedTargetId === target.id &&
            sessionsForTarget(target.id).length
          "
          class="mx-2 mb-2 space-y-1 border-t border-border/60 pt-2"
        >
          <button
            v-for="session in sessionsForTarget(target.id)"
            :key="session.id"
            type="button"
            :class="[
              'flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
              selectedSessionId === session.id
                ? 'bg-background text-foreground shadow-sm'
                : 'text-muted-foreground hover:bg-background/70 hover:text-foreground',
            ]"
            @click="emit('selectSession', session.id)"
          >
            <SquareTerminal class="h-3.5 w-3.5 shrink-0" />
            <span class="min-w-0 flex-1 truncate text-[11px] font-medium">
              {{ session.title }}
            </span>
            <span
              class="inline-flex shrink-0 items-center gap-1 text-[9px]"
              :title="sessionStatus(session).label"
            >
              <span
                :class="[
                  'h-1.5 w-1.5 rounded-full',
                  sessionStatus(session).tone,
                ]"
              />
              <span class="max-w-16 truncate">
                {{ sessionStatus(session).label }}
              </span>
            </span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
