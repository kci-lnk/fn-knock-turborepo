<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { LoaderCircle } from "lucide-vue-next";
import { TerminalAPI } from "@/lib/api/terminal";
import type {
  TerminalAttachmentRecord,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTransport,
} from "../types";
import { useConfigStore } from "../store/config";
import {
  toolbarModifierLabels,
  toolbarNavigationShortcuts,
  toolbarPrimaryShortcuts,
} from "./web-terminal/terminal-runtime";
import { useTerminalInputQueue } from "./web-terminal/useTerminalInputQueue";
import { useTerminalResizeQueue } from "./web-terminal/useTerminalResizeQueue";
import { useTerminalFontSize } from "./web-terminal/useTerminalFontSize";
import { useTerminalContextMenu } from "./web-terminal/useTerminalContextMenu";
import { useTerminalViewportLayout } from "./web-terminal/useTerminalViewportLayout";
import { useTerminalDialogs } from "./web-terminal/useTerminalDialogs";
import { useTerminalSessionController } from "./web-terminal/useTerminalSessionController";
import { useTerminalEmulator } from "./web-terminal/useTerminalEmulator";
import TerminalConnectionErrorAlert from "./web-terminal/TerminalConnectionErrorAlert.vue";
import TerminalContextMenu from "./web-terminal/TerminalContextMenu.vue";
import TerminalGateAlerts from "./web-terminal/TerminalGateAlerts.vue";
import TerminalMobileToolbar from "./web-terminal/TerminalMobileToolbar.vue";
import TerminalRenameDialog from "./web-terminal/TerminalRenameDialog.vue";
import TerminalSendDialog from "./web-terminal/TerminalSendDialog.vue";
import TerminalSessionToolbar from "./web-terminal/TerminalSessionToolbar.vue";
import TerminalWindowChrome from "./web-terminal/TerminalWindowChrome.vue";

const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();
const runtimeStatus = ref<TerminalRuntimeStatus | null>(null);
const sessions = ref<TerminalSessionRecord[]>([]);
const isBooting = ref(true);
const isCreating = ref(false);
const isKilling = ref(false);
const selectedSessionId = ref("");
const connectionState = ref<"idle" | "connecting" | "connected" | "error">(
  "idle",
);
const connectionError = ref("");
const activeTransport = ref<TerminalTransport | null>(null);
const activeAttachment = ref<TerminalAttachmentRecord | null>(null);

let emulator!: ReturnType<typeof useTerminalEmulator>;
const {
  compactViewport,
  isTerminalFullscreen,
  setMobileAccessoryBarRef,
  setTerminalFullscreen,
  setTerminalPanelRef,
  setTerminalShellRef,
  setTerminalStatusRef,
  showMobileAccessoryBar,
  startViewportTracking,
  stopViewportTracking,
  syncViewportHeight,
  terminalFrameRef,
  terminalFrameStyle,
  terminalPanelClass,
  terminalPanelStyle,
  toggleTerminalFullscreen,
} = useTerminalViewportLayout({
  focusTerminal: () => emulator.focusTerminal(),
  scheduleFit: () => emulator.scheduleFit(),
  syncTerminalTextInputAnchor: () => emulator.syncTerminalTextInputAnchor(),
});
const {
  applyTerminalFontSize,
  loadTerminalFontSize,
  nudgeTerminalFontSize,
  persistTerminalFontSize,
  resetTerminalFontSize,
  terminalFontSize,
} = useTerminalFontSize({
  compactViewport,
  getTerminal: () => emulator.getTerminal(),
  scheduleFit: () => emulator.scheduleFit(),
});
const {
  clearPendingInput,
  flushPendingInput,
  getPendingInputSnapshot,
  queueRemoteTerminalResponse,
  queueTerminalInput,
  sendTerminalPayloadNow,
} = useTerminalInputQueue({
  activeAttachment,
  connectionError,
  connectionState,
  selectedSessionId,
  sendInput: (attachmentId, payload) =>
    TerminalAPI.sendInput(attachmentId, payload),
  translate: (key) => t(key),
});
const {
  flushPendingResize,
  markSyncedResize,
  resetResizeState,
  scheduleResize,
} = useTerminalResizeQueue({
  activeAttachment,
  getTerminal: () => emulator.getTerminal(),
  resizeAttachment: (attachmentId, cols, rows) =>
    TerminalAPI.resizeAttachment(attachmentId, cols, rows),
  restartPollingFromSnapshot: (attachment) =>
    restartHttpPollingFromSnapshot(attachment),
  sessions,
});

emulator = useTerminalEmulator({
  applyFontSize: applyTerminalFontSize,
  canAcceptInput: () => Boolean(activeAttachment.value),
  compactViewport,
  persistFontSize: persistTerminalFontSize,
  queueInput: queueTerminalInput,
  queueRemoteResponse: queueRemoteTerminalResponse,
  scheduleResize,
  terminalFontSize,
  terminalFrameRef,
  translate: (key) => t(key),
});
const {
  applyOutputChunk,
  armedModifier,
  clearArmedModifier,
  clearTerminal,
  ensureTerminalReady,
  focusTerminal,
  isPinchZooming,
  resetOutputState,
  terminalMountRef,
  toggleArmedModifier,
} = emulator;
void terminalMountRef;

const selectedSession = computed(
  () =>
    sessions.value.find((session) => session.id === selectedSessionId.value) ||
    null,
);
const terminalWindowTitle = computed(
  () => selectedSession.value?.title?.trim() || t("admin.webTerminal.title"),
);
const terminalWindowSubtitle = computed(() => {
  const session = selectedSession.value;
  const shellSegments = session?.shell.split("/").filter(Boolean) || [];
  const cwdSegments =
    session?.cwd.replace(/\/+$/, "").split("/").filter(Boolean) || [];
  const shell = shellSegments[shellSegments.length - 1] || "shell";
  const cwd = cwdSegments[cwdSegments.length - 1];
  return `${shell} · ${cwd || "~"}`;
});
const destroySessionDescription = computed(() => {
  const title = selectedSession.value?.title?.trim();
  return title
    ? t("admin.webTerminal.destroyDescriptionWithTitle", { title })
    : t("admin.webTerminal.destroyDescription");
});
const terminalEnabled = computed(
  () => configStore.config?.terminal_feature?.enabled === true,
);
const showMobileToolbar = computed(() => {
  if (configStore.config?.terminal_feature?.allow_mobile_toolbar === false) {
    return false;
  }
  return compactViewport.value;
});
const toolbarDisabled = computed(() => !activeAttachment.value);
const armedModifierLabel = computed(() =>
  armedModifier.value ? toolbarModifierLabels[armedModifier.value] : "",
);
const terminalFullscreenLabel = computed(() =>
  isTerminalFullscreen.value
    ? t("admin.webTerminal.exitFullscreen")
    : t("admin.webTerminal.enterFullscreen"),
);
const statusTone = computed(() => {
  if (connectionState.value === "connected") {
    return t("admin.webTerminal.statusConnected");
  }
  if (connectionState.value === "connecting") {
    return t("admin.webTerminal.statusConnecting");
  }
  if (connectionState.value === "error") {
    return t("admin.webTerminal.statusError");
  }
  return t("admin.webTerminal.statusDisconnected");
});
let disposed = false;

const {
  focusTerminalAfterDialogClose,
  isRenamingSession,
  isSendingDialogPayload,
  openManualPasteDialog,
  openRenameDialog,
  openSendDialog,
  renameDialogOpen,
  renameDialogValue,
  sendDialogOpen,
  sendDialogPayload,
  submitRenameDialog,
  submitSendDialog,
} = useTerminalDialogs({
  activeAttachment,
  clearArmedModifier,
  focusTerminal,
  selectedSession,
  sendPayloadNow: sendTerminalPayloadNow,
  sessions,
  translate: (key) => t(key),
  updateSessionTitle: (sessionId, title) =>
    TerminalAPI.updateSessionTitle(sessionId, title),
});

const {
  closeTerminalContextMenu,
  copyTerminalSelectionFromMenu,
  handleDocumentPointerDown,
  handleTerminalContextMenu,
  pasteClipboardToTerminal,
  selectAllTerminalText,
  setTerminalContextMenuRef,
  terminalContextMenuHasSelection,
  terminalContextMenuOpen,
  terminalContextMenuStyle,
} = useTerminalContextMenu({
  activeAttachment,
  clearArmedModifier,
  focusTerminal,
  getTerminal: () => emulator.getTerminal(),
  openManualPasteDialog,
  translate: (key) => t(key),
});

const handleWindowKeydown = (event: KeyboardEvent) => {
  if (event.key !== "Escape") return;
  if (terminalContextMenuOpen.value) {
    event.preventDefault();
    closeTerminalContextMenu();
    focusTerminal();
    return;
  }
  if (!isTerminalFullscreen.value) return;
  event.preventDefault();
  void setTerminalFullscreen(false);
};
const keepTerminalFocused = (event: Event) => {
  if (event instanceof PointerEvent && event.pointerType !== "mouse") return;
  event.preventDefault();
  focusTerminal();
};
const sendToolbarShortcut = (value: string) => {
  clearArmedModifier();
  queueTerminalInput(value, { immediate: true });
  focusTerminal();
};

const {
  bootstrapPage,
  createSession,
  dispose: disposeTerminalSession,
  destroySelectedSession,
  handleSessionTabChange,
  reconnectSession,
  restartHttpPollingFromSnapshot,
} = useTerminalSessionController({
  activeAttachment,
  activeTransport,
  applyOutputChunk,
  clearArmedModifier,
  clearPendingInput,
  clearTerminal,
  connectionError,
  connectionState,
  ensureConfig: async () => {
    if (!configStore.config) await configStore.loadConfig();
  },
  ensureTerminalReady,
  flushPendingInput,
  flushPendingResize,
  getOutputCursor: emulator.getOutputCursor,
  getPendingInputSnapshot,
  getTerminalSize: emulator.getTerminalSize,
  isBooting,
  isCreating,
  isKilling,
  markSyncedResize,
  onBootstrapLayoutReady: syncViewportHeight,
  resetOutputState,
  resetResizeState,
  runtimeStatus,
  scheduleResize,
  selectedSession,
  selectedSessionId,
  sessions,
  terminalEnabled,
});

onMounted(async () => {
  startViewportTracking();
  loadTerminalFontSize();
  await bootstrapPage();
  if (disposed) return;
  window.addEventListener("keydown", handleWindowKeydown);
  document.addEventListener("pointerdown", handleDocumentPointerDown);
});
watch(
  [
    () => sessions.value.length,
    connectionState,
    connectionError,
    showMobileAccessoryBar,
    isTerminalFullscreen,
  ],
  () => {
    void nextTick().then(syncViewportHeight);
  },
);
onBeforeUnmount(() => {
  disposed = true;
  stopViewportTracking();
  window.removeEventListener("keydown", handleWindowKeydown);
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  emulator.dispose();
  void disposeTerminalSession();
});
</script>

<template>
  <div class="flex h-full min-w-0 flex-col gap-3 sm:gap-4">
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
              ref="terminalFrameRef"
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
                  ref="terminalMountRef"
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

    <TerminalSendDialog
      v-model:open="sendDialogOpen"
      v-model:payload="sendDialogPayload"
      :disabled="toolbarDisabled"
      :on-close-auto-focus="focusTerminalAfterDialogClose"
      :sending="isSendingDialogPayload"
      @submit="submitSendDialog"
    />

    <TerminalRenameDialog
      v-model:open="renameDialogOpen"
      v-model:value="renameDialogValue"
      :renaming="isRenamingSession"
      @submit="submitRenameDialog"
    />
  </div>
</template>
