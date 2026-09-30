import { effectScope, nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useTerminalCopy } from "@/views/web-terminal/useTerminalCopy";
import { useTerminalContextMenu } from "@/views/web-terminal/useTerminalContextMenu";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { toast } from "@admin-shared/utils/toast";
import type { TerminalAttachmentRecord } from "@/lib/api/terminal";

vi.mock("@admin-shared/utils/copyTextToClipboard", () => ({
  copyTextToClipboard: vi.fn(),
}));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { info: vi.fn(), success: vi.fn(), error: vi.fn() },
}));
const cleanups: (() => void)[] = [];
afterEach(() => {
  cleanups.splice(0).forEach((f) => f());
  vi.resetAllMocks();
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});
const setup = () => {
  const scope = effectScope();
  cleanups.push(() => scope.stop());
  return scope.run(() => useTerminalCopy((key) => key))!;
};

describe("terminal copy feedback and snapshots", () => {
  it.each(["attachment", "reset", "focus", "newer-paste"])(
    "discards pending paste when invalidated by %s",
    async (reason) => {
      let resolve!: (text: string) => void;
      const readText = vi.fn(
        () =>
          new Promise<string>((done) => {
            resolve = done;
          }),
      );
      vi.stubGlobal("navigator", { clipboard: { readText } });
      const activeAttachment = ref({ id: "old" } as TerminalAttachmentRecord);
      const paste = vi.fn();
      const manualPaste = vi.fn();
      const terminalInput = document.createElement("textarea");
      document.body.append(terminalInput);
      const menu = useTerminalContextMenu({
        activeAttachment,
        clearArmedModifier: vi.fn(),
        focusTerminal: () => terminalInput.focus(),
        getTerminal: () => ({ getSelection: () => "", paste }),
        getTerminalText: () => "",
        copyTerminalText: vi.fn(),
        openManualPasteDialog: manualPaste,
        translate: (key) => key,
      });
      const pending = menu.pasteClipboardToTerminal();
      if (reason === "attachment")
        activeAttachment.value = { id: "new" } as TerminalAttachmentRecord;
      if (reason === "reset") menu.invalidateTerminalContextMenu();
      if (reason === "focus") terminalInput.blur();
      if (reason === "newer-paste") {
        readText.mockResolvedValueOnce("new request");
        await menu.pasteClipboardToTerminal();
      }
      resolve("old command\n");
      await pending;
      expect(paste.mock.calls).toEqual(
        reason === "newer-paste" ? [["new request"]] : [],
      );
      expect(manualPaste).not.toHaveBeenCalled();
    },
  );
  it("does not open a stale manual-paste dialog after clipboard permission is denied", async () => {
    let reject!: (error: Error) => void;
    vi.stubGlobal("navigator", {
      clipboard: {
        readText: () =>
          new Promise((_, fail) => {
            reject = fail;
          }),
      },
    });
    const manualPaste = vi.fn();
    const menu = useTerminalContextMenu({
      activeAttachment: ref({ id: "old" } as TerminalAttachmentRecord),
      clearArmedModifier: vi.fn(),
      focusTerminal: vi.fn(),
      getTerminal: () => ({ getSelection: () => "", paste: vi.fn() }),
      getTerminalText: () => "",
      copyTerminalText: vi.fn(),
      openManualPasteDialog: manualPaste,
      translate: (key) => key,
    });
    const pending = menu.pasteClipboardToTerminal();
    menu.invalidateTerminalContextMenu();
    reject(new Error("denied"));
    await pending;
    expect(manualPaste).not.toHaveBeenCalled();
  });
  it("uses an immutable menu snapshot and clears it on close", async () => {
    let selection = "original log";
    const copy = vi.fn().mockResolvedValue(undefined);
    const menu = useTerminalContextMenu({
      activeAttachment: ref(null),
      clearArmedModifier: vi.fn(),
      focusTerminal: vi.fn(),
      getTerminal: () => ({ getSelection: () => selection, paste: vi.fn() }),
      getTerminalText: () => "all history\nlast line",
      copyTerminalText: copy,
      openManualPasteDialog: vi.fn(),
      translate: (key) => key,
    });
    menu.handleTerminalContextMenu(new MouseEvent("contextmenu"));
    selection = "changed";
    await menu.copyTerminalSelectionFromMenu();
    expect(copy).toHaveBeenLastCalledWith("original log");
    expect(menu.terminalContextMenuHasSelection.value).toBe(false);
    await menu.copyTerminalSelectionFromMenu();
    expect(copy).toHaveBeenLastCalledWith("");
    await menu.copyAllTerminalText();
    expect(copy).toHaveBeenLastCalledWith("all history\nlast line");
  });
  it("offers text for an unverified fallback and opens it automatically on failure", async () => {
    const ui = setup();
    vi.mocked(copyTextToClipboard).mockResolvedValue({
      method: "execCommand",
      verified: false,
    });
    await ui.copyTerminalText("snapshot");
    expect(toast.success).not.toHaveBeenCalled();
    expect(ui.copyDialogOpen.value).toBe(false);
    const action = vi.mocked(toast.info).mock.calls[0]?.[1]?.action;
    if (!action || typeof action !== "object" || !("onClick" in action))
      throw new Error("Missing text action");
    action.onClick(new MouseEvent("click"));
    expect(ui.copyDialogText.value).toBe("snapshot");
    expect(ui.copyDialogOpen.value).toBe(true);
    ui.copyDialogOpen.value = false;
    vi.mocked(copyTextToClipboard).mockRejectedValue(new Error("denied"));
    await ui.copyTerminalText("second snapshot");
    expect(ui.copyDialogOpen.value).toBe(true);
    expect(ui.copyDialogText.value).toBe("second snapshot");
    expect(toast.error).toHaveBeenCalled();
  });
  it("reports writeText completion without claiming readback verification", async () => {
    const ui = setup();
    vi.mocked(copyTextToClipboard).mockResolvedValue({
      method: "clipboard.writeText",
      verified: false,
    });
    await ui.copyTerminalText("text");
    expect(toast.success).toHaveBeenCalledWith(
      "admin.webTerminal.copyCompleted",
    );
    expect(copyTextToClipboard).toHaveBeenCalledWith(
      "text",
      expect.objectContaining({ verify: false, strategy: "write-text-first" }),
    );
  });
  it.each(["reset", "focus", "close"])(
    "does not reopen stale text after %s",
    async (reason) => {
      const ui = setup();
      let reject!: (error: Error) => void;
      vi.mocked(copyTextToClipboard).mockImplementation(
        () =>
          new Promise((_, r) => {
            reject = r;
          }),
      );
      if (reason === "close") ui.copyDialogOpen.value = true;
      const pending = ui.copyTerminalText("old session");
      if (reason === "reset") ui.invalidateCopy();
      if (reason === "close") ui.copyDialogOpen.value = false;
      if (reason === "focus") {
        const field = document.createElement("input");
        document.body.append(field);
        field.focus();
      }
      reject(new Error("denied"));
      await pending;
      await nextTick();
      expect(ui.copyDialogOpen.value).toBe(false);
      expect(toast.error).not.toHaveBeenCalled();
    },
  );
  it("does not write empty selections", async () => {
    const ui = setup();
    await ui.copyTerminalText("\n  ");
    expect(copyTextToClipboard).not.toHaveBeenCalled();
  });
});
