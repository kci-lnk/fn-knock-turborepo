<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  LoaderCircle,
  Laptop,
  LockKeyhole,
  Pencil,
  Plus,
  Server,
  Settings2,
  ShieldAlert,
  SquareTerminal,
} from "lucide-vue-next";
import type {
  TerminalDestination,
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
  targets: TerminalDestination[];
}>();

const emit = defineEmits<{
  add: [];
  edit: [target: TerminalTargetRecord];
  configureLocal: [];
  selectSession: [sessionId: string];
  select: [targetId: string];
}>();

const { t } = useI18n();
const activePhases = new Set([
  "creating",
  "openingPty",
  "startingShell",
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
          {{ t("admin.webTerminal.targets", "Terminal targets") }}
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
        data-terminal-target-row
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
            collapsed ? 'justify-center p-2.5' : 'gap-2.5 px-3 py-2.5 pr-11',
          ]"
          :aria-label="
            target.kind === 'local'
              ? t('admin.webTerminal.localTarget')
              : target.name
          "
          :title="
            collapsed
              ? target.kind === 'local'
                ? `${t('admin.webTerminal.localTarget')} — ${target.executionIdentity}`
                : `${target.name} — ${target.username}@${target.host}`
              : undefined
          "
          @click="emit('select', target.id)"
        >
          <span class="relative shrink-0">
            <Laptop v-if="target.kind === 'local'" class="h-4 w-4" />
            <Server v-else class="h-4 w-4" />
            <LockKeyhole
              v-if="target.kind === 'local' && !target.enabled"
              class="absolute -bottom-1 -right-1 h-2.5 w-2.5 rounded-full bg-background text-amber-600"
            />
            <span
              v-if="activeCount(target.id)"
              class="absolute -right-1 -top-1 h-2 w-2 rounded-full border border-background bg-emerald-500"
            />
          </span>
          <span v-if="!collapsed" class="min-w-0 flex-1">
            <span class="flex items-center gap-1.5">
              <span class="truncate text-xs font-medium">{{
                target.kind === "local"
                  ? t("admin.webTerminal.localTarget")
                  : target.name
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
              <template v-if="target.kind === 'local'">
                {{ target.executionIdentity }} ·
                {{
                  target.enabled
                    ? t("admin.webTerminal.localReady")
                    : t("admin.webTerminal.localLocked")
                }}
              </template>
              <template v-else>
                {{ target.username }}@{{ target.host }}:{{ target.port }}
              </template>
            </span>
          </span>
        </button>

        <div
          v-if="!collapsed"
          class="terminal-target-actions absolute right-1.5 top-1.5 flex transition-opacity"
        >
          <Button
            v-if="target.kind === 'local'"
            size="icon-sm"
            variant="ghost"
            :class="target.privileged ? 'text-amber-600' : ''"
            :aria-label="t('admin.webTerminal.localSettingsTitle')"
            :title="t('admin.webTerminal.localSettingsTitle')"
            @click.stop="emit('configureLocal')"
          >
            <ShieldAlert v-if="target.privileged" class="h-3.5 w-3.5" />
            <Settings2 v-else class="h-3.5 w-3.5" />
          </Button>
          <Button
            v-if="target.kind === 'ssh'"
            size="icon-sm"
            variant="ghost"
            :aria-label="t('common.edit')"
            :title="t('common.edit')"
            @click.stop="emit('edit', target)"
          >
            <Pencil class="h-3.5 w-3.5" />
          </Button>
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

<style scoped>
/* Touch and hybrid devices expose actions without consuming the first tap. */
@media (hover: hover) and (pointer: fine) and (not (any-pointer: coarse)) {
  .terminal-target-actions {
    visibility: hidden;
    pointer-events: none;
    opacity: 0;
  }

  [data-terminal-target-row]:hover > .terminal-target-actions,
  [data-terminal-target-row]:focus-within > .terminal-target-actions {
    visibility: visible;
    pointer-events: auto;
    opacity: 1;
  }
}
</style>
