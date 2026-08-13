<script setup lang="ts">
import { LoaderCircle } from "lucide-vue-next";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  toolbarModifierLabels,
  toolbarNavigationShortcuts,
  toolbarPrimaryShortcuts,
} from "./terminal-runtime";
import TerminalConnectionErrorAlert from "./TerminalConnectionErrorAlert.vue";
import TerminalContextMenu from "./TerminalContextMenu.vue";
import TerminalGateAlerts from "./TerminalGateAlerts.vue";
import TerminalMobileToolbar from "./TerminalMobileToolbar.vue";
import TerminalSessionToolbar from "./TerminalSessionToolbar.vue";
import TerminalWindowChrome from "./TerminalWindowChrome.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  activeAttachment,
  armedModifier,
  armedModifierLabel,
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
  pasteClipboardToTerminal,
  reconnectSession,
  resetTerminalFontSize,
  router,
  runtimeStatus,
  selectAllTerminalText,
  selectedSession,
  selectedSessionId,
  sendToolbarShortcut,
  sessions,
  setMobileAccessoryBarRef,
  setTerminalContextMenuRef,
  setTerminalFrameElement,
  setTerminalMountElement,
  setTerminalPanelRef,
  setTerminalShellRef,
  setTerminalStatusRef,
  showMobileAccessoryBar,
  showMobileToolbar,
  statusTone,
  t,
  terminalContextMenuHasSelection,
  terminalContextMenuOpen,
  terminalContextMenuStyle,
  terminalEnabled,
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
<TerminalGateAlerts
      v-if="!terminalEnabled || runtimeStatus?.blockedReason"
      :blocked-reason="runtimeStatus?.blockedReason || ''"
      :terminal-enabled="terminalEnabled"
      @go-settings="router.push('/system?tab=terminal')"
    />

    <div
      v-else
      :ref="setTerminalShellRef"
      class="min-h-0 min-w-0 md:min-h-[80vh]"
    >
      <Card class="min-h-0 min-w-0 h-full py-3 sm:py-6">
        <CardContent
          class="flex h-full min-h-0 min-w-0 flex-col gap-2.5 px-3 sm:gap-3 sm:px-6"
        >
          <TerminalSessionToolbar
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
            :reconnect-session="reconnectSession"
            :selected-session="selectedSession"
            :selected-session-id="selectedSessionId"
            :sessions="sessions"
            :status-tone="statusTone"
            :toolbar-disabled="toolbarDisabled"
          />

          <div v-if="isBooting" class="space-y-3">
            <Skeleton class="h-12 w-full rounded-xl" />
            <Skeleton class="h-14 w-full rounded-xl" />
          </div>

          <div
            v-else-if="sessions.length === 0"
            class="rounded-xl border border-dashed border-border/80 bg-muted/10 px-4 py-5 text-sm text-muted-foreground"
          >
            {{ t("admin.webTerminal.noDefaultSession") }}
          </div>

          <TerminalConnectionErrorAlert :message="connectionError" />

          <div
            v-if="!isBooting && sessions.length > 0"
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
                <!-- The mounted terminal owns keyboard focus and dispatches
                     Shift+F10/context-menu events through this capture boundary. -->
                <!-- eslint-disable-next-line vuejs-accessibility/no-static-element-interactions -->
                <div
                  :ref="setTerminalMountElement"
                  class="absolute inset-0 box-border min-h-0 min-w-0 overflow-hidden px-1.5 py-2.5 sm:px-2.5 sm:py-3"
                  @contextmenu.capture="handleTerminalContextMenu"
                />
                <TerminalContextMenu
                  :ref="setTerminalContextMenuRef"
                  :can-paste="!!activeAttachment"
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
              class="shrink-0 flex flex-col gap-2 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between"
            >
              <span
                v-if="connectionState === 'connecting'"
                class="inline-flex items-center gap-1.5"
              >
                <LoaderCircle class="h-3.5 w-3.5 animate-spin" />
                <span>{{ t("admin.webTerminal.connecting") }}</span>
              </span>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
</template>
