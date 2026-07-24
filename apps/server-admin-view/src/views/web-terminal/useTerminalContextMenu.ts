import {
  computed,
  nextTick,
  ref,
  type ComponentPublicInstance,
  type Ref,
} from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { TerminalAttachmentRecord } from "@/types";
import {
  TERMINAL_CONTEXT_MENU_HEIGHT,
  TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
  TERMINAL_CONTEXT_MENU_WIDTH,
} from "./terminal-runtime";
import {
  copyTextToClipboard,
  focusElementWithoutScroll,
  resolveConstrainedMenuPosition,
} from "./terminal-dom";

type TerminalSelectionApi = {
  focus?: () => void;
  getSelection: () => string;
  paste: (text: string) => void;
  selectAll: () => void;
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
  focusTerminal,
  getTerminal,
  openManualPasteDialog,
  translate,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  clearArmedModifier: () => void;
  focusTerminal: () => void;
  getTerminal: () => TerminalSelectionApi | null;
  openManualPasteDialog: () => void;
  translate: (key: string) => string;
}) => {
  const terminalContextMenuRef = ref<TerminalContextMenuHandle | null>(null);
  const terminalContextMenuOpen = ref(false);
  const terminalContextMenuX = ref(0);
  const terminalContextMenuY = ref(0);
  const terminalContextMenuHasSelection = ref(false);

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
      const root = terminalContextMenuRef.value?.rootElement;
      focusElementWithoutScroll(
        root?.querySelector<HTMLButtonElement>("button:not(:disabled)") ||
          root ||
          document.body,
      );
    });
  };

  const copyTerminalSelectionFromMenu = async () => {
    const selectedText = getTerminal()?.getSelection() || "";
    closeTerminalContextMenu();

    if (!selectedText.length) {
      toast.info(translate("admin.webTerminal.noSelection"));
      focusTerminal();
      return;
    }

    try {
      await copyTextToClipboard(selectedText);
      toast.success(translate("admin.webTerminal.selectionCopied"));
    } catch (error) {
      toast.error(translate("admin.webTerminal.copyFailed"), {
        description:
          error instanceof Error
            ? error.message
            : translate("admin.webTerminal.copySelectionFailed"),
      });
    } finally {
      focusTerminal();
    }
  };

  const pasteClipboardToTerminal = async () => {
    closeTerminalContextMenu();

    if (!activeAttachment.value) {
      toast.error(translate("admin.webTerminal.noConnection"));
      focusTerminal();
      return;
    }

    try {
      const text = await readTextFromClipboard(translate);
      if (!text) {
        toast.info(translate("admin.webTerminal.emptyClipboard"));
        focusTerminal();
        return;
      }

      clearArmedModifier();
      getTerminal()?.paste(text);
      focusTerminal();
    } catch (error) {
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

  const selectAllTerminalText = () => {
    closeTerminalContextMenu();
    getTerminal()?.selectAll();
    terminalContextMenuHasSelection.value = true;
    focusTerminal();
  };

  return {
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
  };
};
