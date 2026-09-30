import {
  computed,
  nextTick,
  ref,
  type ComponentPublicInstance,
  type Ref,
} from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { TerminalAttachmentRecord } from "@/lib/api/terminal";
import {
  TERMINAL_CONTEXT_MENU_HEIGHT,
  TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
  TERMINAL_CONTEXT_MENU_WIDTH,
} from "./terminal-runtime";
import {
  focusElementWithoutScroll,
  resolveConstrainedMenuPosition,
} from "./terminal-dom";

type TerminalSelectionApi = {
  focus?: () => void;
  getSelection: () => string;
  paste: (text: string) => void;
};

type TerminalContextMenuHandle = {
  rootElement?: HTMLElement | null;
};

const readTextFromClipboard = async (
  translate: (key: string) => string,
): Promise<string> => {
  if (typeof navigator !== "undefined" && navigator.clipboard?.readText) {
    return navigator.clipboard.readText();
  }

  throw new Error(translate("admin.webTerminal.clipboardPermissionDenied"));
};

export const useTerminalContextMenu = ({
  activeAttachment,
  clearArmedModifier,
  copyTerminalText,
  focusTerminal,
  getTerminal,
  getTerminalText,
  openManualPasteDialog,
  translate,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  clearArmedModifier: () => void;
  copyTerminalText: (text: string) => Promise<void>;
  focusTerminal: () => void;
  getTerminal: () => TerminalSelectionApi | null;
  getTerminalText: () => string;
  openManualPasteDialog: () => void;
  translate: (key: string) => string;
}) => {
  const terminalContextMenuRef = ref<TerminalContextMenuHandle | null>(null);
  const terminalContextMenuOpen = ref(false);
  const terminalContextMenuX = ref(0);
  const terminalContextMenuY = ref(0);
  const terminalContextMenuHasSelection = ref(false);
  let selectionSnapshot = "";
  let pasteGeneration = 0;

  const setTerminalContextMenuRef = (
    instance: Element | ComponentPublicInstance | null,
  ) => {
    terminalContextMenuRef.value = instance as TerminalContextMenuHandle | null;
  };

  const terminalContextMenuStyle = computed(() => ({
    left: `${terminalContextMenuX.value}px`,
    top: `${terminalContextMenuY.value}px`,
  }));

  const closeTerminalContextMenu = () => {
    terminalContextMenuOpen.value = false;
    terminalContextMenuHasSelection.value = false;
    selectionSnapshot = "";
  };

  const invalidateTerminalContextMenu = () => {
    pasteGeneration += 1;
    closeTerminalContextMenu();
  };

  const handleDocumentPointerDown = (event: PointerEvent) => {
    if (!terminalContextMenuOpen.value) return;

    const target = event.target;
    if (
      target instanceof Node &&
      terminalContextMenuRef.value?.rootElement?.contains(target)
    ) {
      return;
    }

    closeTerminalContextMenu();
  };

  const handleTerminalContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();

    const selectedText = getTerminal()?.getSelection() || "";
    selectionSnapshot = selectedText;
    const viewportWidth = window.innerWidth || TERMINAL_CONTEXT_MENU_WIDTH;
    const viewportHeight = window.innerHeight || TERMINAL_CONTEXT_MENU_HEIGHT;
    const menuPosition = resolveConstrainedMenuPosition({
      clientX: event.clientX,
      clientY: event.clientY,
      menuHeight: TERMINAL_CONTEXT_MENU_HEIGHT,
      menuWidth: TERMINAL_CONTEXT_MENU_WIDTH,
      viewportGap: TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
      viewportHeight,
      viewportWidth,
    });

    terminalContextMenuHasSelection.value = selectedText.length > 0;
    terminalContextMenuX.value = menuPosition.x;
    terminalContextMenuY.value = menuPosition.y;
    terminalContextMenuOpen.value = true;
    void nextTick(() => {
      if (!terminalContextMenuOpen.value) return;
      const root = terminalContextMenuRef.value?.rootElement;
      focusElementWithoutScroll(
        root?.querySelector<HTMLButtonElement>("button:not(:disabled)") ||
          root ||
          document.body,
      );
    });
  };

  const copyTerminalSelectionFromMenu = async () => {
    const selectedText = selectionSnapshot;
    closeTerminalContextMenu();
    focusTerminal();
    await copyTerminalText(selectedText);
  };

  const pasteClipboardToTerminal = async () => {
    const operation = ++pasteGeneration;
    const attachment = activeAttachment.value;
    closeTerminalContextMenu();
    focusTerminal();
    const origin = document.activeElement;
    const isCurrent = () =>
      operation === pasteGeneration &&
      attachment === activeAttachment.value &&
      document.activeElement === origin;

    if (!attachment) {
      toast.error(translate("admin.webTerminal.noConnection"));
      return;
    }

    try {
      const text = await readTextFromClipboard(translate);
      if (!isCurrent()) return;
      if (!text) {
        toast.info(translate("admin.webTerminal.emptyClipboard"));
        focusTerminal();
        return;
      }

      clearArmedModifier();
      getTerminal()?.paste(text);
      focusTerminal();
    } catch (error) {
      if (!isCurrent()) return;
      console.warn(
        "[terminal] clipboard read unavailable, using manual paste",
        {
          error:
            error instanceof Error ? error.message : String(error ?? "unknown"),
        },
      );
      openManualPasteDialog();
    }
  };

  const copyAllTerminalText = () => {
    const text = getTerminalText();
    closeTerminalContextMenu();
    focusTerminal();
    return copyTerminalText(text);
  };

  return {
    closeTerminalContextMenu,
    invalidateTerminalContextMenu,
    copyTerminalSelectionFromMenu,
    handleDocumentPointerDown,
    handleTerminalContextMenu,
    pasteClipboardToTerminal,
    copyAllTerminalText,
    setTerminalContextMenuRef,
    terminalContextMenuHasSelection,
    terminalContextMenuOpen,
    terminalContextMenuStyle,
  };
};
