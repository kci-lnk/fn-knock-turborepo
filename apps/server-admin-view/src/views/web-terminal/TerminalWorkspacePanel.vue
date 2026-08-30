<script setup lang="ts">
import {
  Eye,
  Laptop,
  LoaderCircle,
  LockKeyhole,
  Plus,
  Server,
  ShieldAlert,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  toolbarModifierLabels,
  toolbarNavigationShortcuts,
  toolbarPrimaryShortcuts,
} from "./terminal-runtime";
import TerminalConnectionErrorAlert from "./TerminalConnectionErrorAlert.vue";
import TerminalContextMenu from "./TerminalContextMenu.vue";
import TerminalMobileToolbar from "./TerminalMobileToolbar.vue";
import TerminalSessionToolbar from "./TerminalSessionToolbar.vue";
import TerminalWindowChrome from "./TerminalWindowChrome.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  activeAttachment,
  armedModifier,
  armedModifierLabel,
  canClaimControl,
  claimControl,
  closeTerminalContextMenu,
  connectionError,
  connectionState,
  copyTerminalSelectionFromMenu,
  createSession,
  destroySelectedSession,
  destroySessionDescription,
  handleSessionTabChange,
  handleTerminalContextMenu,
  isBooting,
  isCreating,
  isKilling,
  isPinchZooming,
  isRenamingSession,
  isTerminalFullscreen,
  keepTerminalFocused,
  nudgeTerminalFontSize,
  openRenameDialog,
  openSendDialog,
  openTargetCreate,
  openLocalSettings,
  pasteClipboardToTerminal,
  readOnly,
  reconnectSession,
  resetTerminalFontSize,
  runtimeRestarted,
  selectAllTerminalText,
  selectedSession,
  selectedSessionId,
  selectedTarget,
  sendToolbarShortcut,
  sessionsForTarget,
  setMobileAccessoryBarRef,
  setTargetDrawerOpen,
  setTerminalContextMenuRef,
  setTerminalFrameElement,
  setTerminalMountElement,
  setTerminalPanelRef,
  setTerminalStatusRef,
  showMobileAccessoryBar,
  showMobileToolbar,
  statusTone,
  t,
  terminalContextMenuHasSelection,
  terminalContextMenuOpen,
  terminalContextMenuStyle,
  terminalFontSize,
  terminalFrameStyle,
  terminalFullscreenLabel,
  terminalPanelClass,
  terminalPanelStyle,
  terminalWindowSubtitle,
  terminalWindowTitle,
  toggleArmedModifier,
  toggleTerminalFullscreen,
  toolbarDisabled,
} = props.controller;
</script>

<template>
  <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-2.5 p-3 sm:gap-3 sm:p-6">
    <TerminalSessionToolbar
      :claim-control="claimControl"
      :can-claim-control="canClaimControl"
      :connection-state="connectionState"
      :create-session="createSession"
      :destroy-selected-session="destroySelectedSession"
      :destroy-session-description="destroySessionDescription"
      :handle-session-tab-change="handleSessionTabChange"
      :is-booting="isBooting"
      :is-creating="isCreating"
      :is-killing="isKilling"
      :is-renaming-session="isRenamingSession"
      :keep-terminal-focused="keepTerminalFocused"
      :open-rename-dialog="openRenameDialog"
      :open-send-dialog="openSendDialog"
      :open-target-drawer="() => setTargetDrawerOpen(true)"
      :reconnect-session="reconnectSession"
      :selected-session="selectedSession"
      :selected-session-id="selectedSessionId"
      :selected-target="selectedTarget"
      :sessions="sessionsForTarget"
      :status-tone="statusTone"
      :toolbar-disabled="toolbarDisabled"
    />

    <div v-if="isBooting" class="space-y-3">
      <Skeleton class="h-12 w-full rounded-xl" />
      <Skeleton class="h-[420px] w-full rounded-xl" />
    </div>

    <div
      v-else-if="!selectedTarget"
      class="grid min-h-[360px] place-items-center rounded-2xl border border-dashed border-border/80 bg-muted/10 p-6 text-center"
    >
      <div class="max-w-sm">
        <Server class="mx-auto h-8 w-8 text-muted-foreground" />
        <h2 class="mt-3 text-base font-semibold">
          {{ t("admin.webTerminal.noTargets", "No SSH targets") }}
        </h2>
        <p class="mt-1 text-sm leading-6 text-muted-foreground">
          {{
            t(
              "admin.webTerminal.noTargetsDescription",
              "Add a server, verify its fingerprint, and test authentication before opening a shell.",
            )
          }}
        </p>
        <Button class="mt-4" @click="openTargetCreate">
          <Plus class="mr-1.5 h-4 w-4" />
          {{ t("admin.webTerminal.addTarget", "Add SSH target") }}
        </Button>
      </div>
    </div>

    <div
      v-else-if="
        selectedTarget.kind === 'local' &&
        (!selectedTarget.enabled || !selectedTarget.ready)
      "
      class="grid min-h-[360px] place-items-center rounded-2xl border border-dashed border-amber-500/30 bg-amber-500/5 p-6 text-center"
    >
      <div class="max-w-md">
        <span
          class="mx-auto inline-flex h-12 w-12 items-center justify-center rounded-2xl bg-amber-500/10 text-amber-700 dark:text-amber-300"
        >
          <LockKeyhole v-if="!selectedTarget.enabled" class="h-6 w-6" />
          <ShieldAlert v-else class="h-6 w-6" />
        </span>
        <h2 class="mt-3 text-base font-semibold">
          {{
            selectedTarget.enabled
              ? t("admin.webTerminal.localUnavailableTitle")
              : t("admin.webTerminal.localLockedTitle")
          }}
        </h2>
        <p class="mt-1 text-sm leading-6 text-muted-foreground">
          {{
            selectedTarget.enabled
              ? t("admin.webTerminal.localUnavailableDescription")
              : t("admin.webTerminal.localLockedDescription", {
                  identity: selectedTarget.executionIdentity,
                })
          }}
        </p>
        <Button class="mt-4" @click="openLocalSettings">
          <ShieldAlert class="mr-1.5 h-4 w-4" />
          {{ t("admin.webTerminal.localSettingsTitle") }}
        </Button>
      </div>
    </div>

    <div
      v-else-if="!selectedSession"
      class="grid min-h-[360px] place-items-center rounded-2xl border border-dashed border-border/80 bg-muted/10 p-6 text-center"
    >
      <div class="max-w-sm">
        <Laptop
          v-if="selectedTarget.kind === 'local'"
          class="mx-auto h-8 w-8 text-muted-foreground"
        />
        <Server v-else class="mx-auto h-8 w-8 text-muted-foreground" />
        <h2 class="mt-3 text-base font-semibold">
          {{ t("admin.webTerminal.noSessions", "No sessions on this target") }}
        </h2>
        <p class="mt-1 text-sm leading-6 text-muted-foreground">
          {{
            selectedTarget.kind === "local"
              ? t("admin.webTerminal.localNoSessionsDescription")
              : t(
                  "admin.webTerminal.noSessionsDescription",
                  "Create an independent SSH shell. It stays alive while the terminal service is running.",
                )
          }}
        </p>
        <Button class="mt-4" :disabled="isCreating" @click="createSession">
          <LoaderCircle v-if="isCreating" class="mr-1.5 h-4 w-4 animate-spin" />
          <Plus v-else class="mr-1.5 h-4 w-4" />
          {{ t("admin.webTerminal.newSession") }}
        </Button>
      </div>
    </div>

    <div
      v-if="selectedSession && runtimeRestarted"
      class="rounded-lg border border-amber-500/25 bg-amber-500/5 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
    >
      {{
        t(
          "admin.webTerminal.runtimeRestarted",
          "The terminal service restarted. Previous sessions have ended.",
        )
      }}
    </div>
    <div
      v-if="selectedSession && canClaimControl"
      class="flex items-center gap-2 rounded-lg border border-sky-500/25 bg-sky-500/5 px-3 py-2 text-xs text-sky-700 dark:text-sky-300"
    >
      <Eye class="h-3.5 w-3.5" />
      {{
        t(
          "admin.webTerminal.viewerDescription",
          "Another browser controls this session. You can watch output or take control.",
        )
      }}
    </div>

    <TerminalConnectionErrorAlert
      v-if="selectedSession"
      :message="connectionError"
    />

    <div
      v-show="!isBooting && selectedTarget && selectedSession"
      :ref="setTerminalPanelRef"
      :class="[
        'flex min-h-0 flex-1 flex-col gap-2.5 sm:gap-3',
        terminalPanelClass,
      ]"
      :style="terminalPanelStyle"
    >
      <div
        :ref="setTerminalFrameElement"
        :class="[
          'relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-[18px] border bg-[#1d1d1f] shadow-[0_14px_34px_rgba(15,23,42,0.18)] transition-[box-shadow,border-color] duration-200',
          isPinchZooming
            ? 'border-cyan-400/65 shadow-[0_0_0_1px_rgba(34,211,238,0.26),0_16px_38px_rgba(8,145,178,0.14)]'
            : 'border-white/8',
        ]"
        :style="terminalFrameStyle"
        :title="terminalWindowSubtitle"
      >
        <TerminalWindowChrome
          :fullscreen="isTerminalFullscreen"
          :fullscreen-label="terminalFullscreenLabel"
          :title="terminalWindowTitle"
          :toggle-fullscreen="toggleTerminalFullscreen"
        />

        <div class="relative min-h-0 flex-1 bg-[#1c1c1e]">
          <div
            class="pointer-events-none absolute inset-x-0 top-0 h-px bg-white/4"
          />
          <!-- eslint-disable-next-line vuejs-accessibility/no-static-element-interactions -->
          <div
            :ref="setTerminalMountElement"
            class="absolute inset-0 box-border min-h-0 min-w-0 overflow-hidden px-1.5 py-2.5 sm:px-2.5 sm:py-3"
            @contextmenu.capture="handleTerminalContextMenu"
          />
          <TerminalContextMenu
            :ref="setTerminalContextMenuRef"
            :can-paste="!!activeAttachment && !readOnly"
            :has-selection="terminalContextMenuHasSelection"
            :menu-style="terminalContextMenuStyle"
            :open="terminalContextMenuOpen"
            @close="closeTerminalContextMenu"
            @copy="copyTerminalSelectionFromMenu"
            @paste="pasteClipboardToTerminal"
            @select-all="selectAllTerminalText"
          />
        </div>
      </div>

      <div
        v-if="showMobileAccessoryBar"
        :ref="setMobileAccessoryBarRef"
        class="shrink-0 rounded-2xl border border-border/70 bg-muted/15 p-2 pb-[calc(env(safe-area-inset-bottom)+0.5rem)] sm:p-2.5 sm:pb-2.5"
      >
        <TerminalMobileToolbar
          :armed-modifier="armedModifier"
          :armed-modifier-label="armedModifierLabel"
          :disabled="toolbarDisabled"
          :font-size="terminalFontSize"
          :keep-focused="keepTerminalFocused"
          :modifier-labels="toolbarModifierLabels"
          :navigation-shortcuts="toolbarNavigationShortcuts"
          :nudge-font-size="nudgeTerminalFontSize"
          :primary-shortcuts="toolbarPrimaryShortcuts"
          :reset-font-size="resetTerminalFontSize"
          :send-shortcut="sendToolbarShortcut"
          :show="showMobileToolbar"
          :toggle-modifier="toggleArmedModifier"
        />
      </div>

      <div
        :ref="setTerminalStatusRef"
        class="flex shrink-0 flex-col gap-2 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between"
      >
        <span class="truncate">{{ terminalWindowSubtitle }}</span>
        <span
          v-if="connectionState === 'connecting'"
          class="inline-flex items-center gap-1.5"
        >
          <LoaderCircle class="h-3.5 w-3.5 animate-spin" />
          <span>{{ statusTone }}</span>
        </span>
      </div>
    </div>
  </div>
</template>
