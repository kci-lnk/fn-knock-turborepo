<script setup lang="ts">
import { useI18n } from "vue-i18n";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import {
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCcw,
  Send,
  Trash2,
} from "lucide-vue-next";
import type { TerminalSessionRecord } from "../../types";

defineProps<{
  connectionState: "idle" | "connecting" | "connected" | "error";
  createSession: () => Promise<TerminalSessionRecord | null> | void;
  destroySelectedSession: () => Promise<void> | void;
  destroySessionDescription: string;
  handleSessionTabChange: (sessionId: string | number) => Promise<void> | void;
  isBooting: boolean;
  isCreating: boolean;
  isKilling: boolean;
  isRenamingSession: boolean;
  keepTerminalFocused: (event: Event) => void;
  openRenameDialog: () => void;
  openSendDialog: () => void;
  reconnectSession: () => Promise<void> | void;
  selectedSession: TerminalSessionRecord | null;
  selectedSessionId: string;
  sessions: TerminalSessionRecord[];
  statusTone: string;
  toolbarDisabled: boolean;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="shrink-0 flex flex-col gap-2.5 lg:flex-row lg:items-center">
    <div class="flex flex-wrap items-center gap-2">
      <div class="flex items-center gap-2 pl-2">
        <LiveStatusBadge
          v-if="connectionState === 'connected'"
          :active="true"
          :active-label="t('admin.webTerminal.statusConnected')"
          class="mt-px mr-3"
        />
        <span
          v-else
          :aria-label="statusTone"
          :title="statusTone"
          class="inline-flex h-2 w-2 shrink-0 rounded-full bg-zinc-300 align-middle"
          role="status"
        />

        <Button
          variant="outline"
          size="icon-sm"
          class="rounded-lg border-border/70 bg-background/85 shadow-none"
          :disabled="isCreating || isBooting"
          :aria-label="t('admin.webTerminal.newSessionAria')"
          :title="t('admin.webTerminal.newSession')"
          @click="createSession"
        >
          <LoaderCircle v-if="isCreating" class="h-4 w-4 animate-spin" />
          <Plus v-else class="h-4 w-4" />
          <span class="sr-only">{{ t("admin.webTerminal.newSession") }}</span>
        </Button>
        <Button
          variant="outline"
          size="icon-sm"
          class="rounded-lg border-border/70 bg-background/85 shadow-none"
          :disabled="!selectedSession || isRenamingSession"
          :aria-label="t('admin.webTerminal.renameSession')"
          :title="t('admin.webTerminal.renameSession')"
          @click="openRenameDialog"
        >
          <LoaderCircle
            v-if="isRenamingSession"
            class="h-4 w-4 animate-spin"
          />
          <Pencil v-else class="h-4 w-4" />
          <span class="sr-only">{{ t("admin.webTerminal.renameSession") }}</span>
        </Button>
        <Button
          variant="outline"
          size="icon-sm"
          class="rounded-lg border-border/70 bg-background/85 shadow-none"
          :disabled="!selectedSession || connectionState === 'connecting'"
          :aria-label="t('admin.webTerminal.reconnectAria')"
          :title="t('admin.webTerminal.reconnect')"
          @pointerdown="keepTerminalFocused"
          @click="reconnectSession"
        >
          <RefreshCcw class="h-4 w-4" />
          <span class="sr-only">{{ t("admin.webTerminal.reconnect") }}</span>
        </Button>
        <Button
          variant="outline"
          size="icon-sm"
          class="rounded-lg border-border/70 bg-background/85 shadow-none"
          :disabled="toolbarDisabled"
          :aria-label="t('admin.webTerminal.sendAria')"
          :title="t('admin.webTerminal.send')"
          @click="openSendDialog"
        >
          <Send class="h-4 w-4" />
          <span class="sr-only">{{ t("admin.webTerminal.send") }}</span>
        </Button>
      </div>

      <div class="h-8 w-px shrink-0 bg-border/70" />

      <ConfirmDangerPopover
        :title="t('admin.webTerminal.endConfirmTitle')"
        :description="destroySessionDescription"
        :confirm-text="t('admin.webTerminal.endSession')"
        :loading="isKilling"
        :disabled="!selectedSession || isKilling"
        :on-confirm="destroySelectedSession"
        content-class="w-72 text-left"
      >
        <template #trigger>
          <Button
            variant="ghost"
            size="icon-sm"
            class="rounded-lg text-destructive hover:bg-destructive/10 hover:text-destructive"
            :disabled="!selectedSession || isKilling"
            :aria-label="t('admin.webTerminal.endCurrentSession')"
            :title="t('admin.webTerminal.endSession')"
          >
            <Trash2 class="h-4 w-4" />
            <span class="sr-only">{{ t("admin.webTerminal.endSession") }}</span>
          </Button>
        </template>
      </ConfirmDangerPopover>
    </div>

    <div
      v-if="sessions.length > 1"
      class="h-px w-full shrink-0 bg-border/70 lg:h-9 lg:w-px"
    />

    <Tabs
      v-if="sessions.length > 1"
      :model-value="selectedSessionId"
      class="min-w-0 flex-1"
      @update:model-value="handleSessionTabChange"
    >
      <div
        class="overflow-x-auto pb-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden lg:pb-0"
      >
        <TabsList
          class="inline-flex h-9 min-w-max items-center gap-1 rounded-lg border border-border/70 bg-background/72 p-1 lg:ml-auto"
        >
          <TabsTrigger
            v-for="session in sessions"
            :key="session.id"
            :value="session.id"
            class="h-7 min-w-[92px] max-w-[148px] rounded-md px-2.5 text-[11px] font-medium sm:min-w-[110px] sm:max-w-[180px] sm:text-xs"
          >
            <span class="truncate">{{ session.title }}</span>
          </TabsTrigger>
        </TabsList>
      </div>
    </Tabs>
  </div>
</template>
