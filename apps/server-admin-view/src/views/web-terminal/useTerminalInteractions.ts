import type { ComputedRef, Ref } from "vue";
import type {
  TerminalAttachmentRecord,
  TerminalSessionRecord,
} from "@/lib/api/terminal";
import { useTerminalContextMenu } from "./useTerminalContextMenu";
import { useTerminalDialogs } from "./useTerminalDialogs";
import type { useTerminalEmulator } from "./useTerminalEmulator";
import type { useTerminalInputQueue } from "./useTerminalInputQueue";

export const useTerminalInteractions = ({
  activeAttachment,
  cancelRenameSession,
  emulator,
  inputQueue,
  isTerminalFullscreen,
  renameSession,
  selectedSession,
  sessions,
  setTerminalFullscreen,
  translate,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  cancelRenameSession: () => void;
  emulator: ReturnType<typeof useTerminalEmulator>;
  inputQueue: ReturnType<typeof useTerminalInputQueue>;
  isTerminalFullscreen: Ref<boolean>;
  renameSession: (
    sessionId: string,
    title: string,
  ) => Promise<TerminalSessionRecord>;
  selectedSession: ComputedRef<TerminalSessionRecord | null>;
  sessions: Ref<TerminalSessionRecord[]>;
  setTerminalFullscreen: (fullscreen: boolean) => Promise<void>;
  translate: (key: string) => string;
}) => {
  const dialogs = useTerminalDialogs({
    activeAttachment,
    cancelRenameSession,
    clearArmedModifier: emulator.clearArmedModifier,
    focusTerminal: emulator.focusTerminal,
    selectedSession,
    sendPayloadNow: inputQueue.sendTerminalPayloadNow,
    sessions,
    translate,
    updateSessionTitle: renameSession,
  });
  const contextMenu = useTerminalContextMenu({
    activeAttachment,
    clearArmedModifier: emulator.clearArmedModifier,
    focusTerminal: emulator.focusTerminal,
    getTerminal: emulator.getTerminal,
    openManualPasteDialog: dialogs.openManualPasteDialog,
    translate,
  });

  const handleWindowKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    if (contextMenu.terminalContextMenuOpen.value) {
      event.preventDefault();
      contextMenu.closeTerminalContextMenu();
      emulator.focusTerminal();
      return;
    }
    if (!isTerminalFullscreen.value) return;
    event.preventDefault();
    void setTerminalFullscreen(false);
  };

  const keepTerminalFocused = (event: Event) => {
    if (event instanceof PointerEvent && event.pointerType !== "mouse") return;
    event.preventDefault();
    emulator.focusTerminal();
  };
  const sendToolbarShortcut = (value: string) => {
    emulator.clearArmedModifier();
    inputQueue.queueTerminalInput(value, { immediate: true });
    emulator.focusTerminal();
  };
  const start = () => {
    window.addEventListener("keydown", handleWindowKeydown);
    document.addEventListener(
      "pointerdown",
      contextMenu.handleDocumentPointerDown,
    );
  };
  const stop = () => {
    window.removeEventListener("keydown", handleWindowKeydown);
    document.removeEventListener(
      "pointerdown",
      contextMenu.handleDocumentPointerDown,
    );
  };

  return {
    ...contextMenu,
    ...dialogs,
    keepTerminalFocused,
    sendToolbarShortcut,
    start,
    stop,
  };
};
