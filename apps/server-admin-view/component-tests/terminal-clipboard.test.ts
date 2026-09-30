import { Ghostty, InputHandler } from "ghostty-web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bindTerminalClipboard } from "@/views/web-terminal/terminal-clipboard";
import { bindTerminalTextInput } from "@/views/web-terminal/terminal-text-input";
import { snapshotTerminalText } from "@/views/web-terminal/terminal-buffer-text";

let ghostty: Ghostty;
const cleanups: (() => void)[] = [];
beforeEach(async () => {
  ghostty = await Ghostty.load();
});
afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
});

function setup(selection = "selected log") {
  const mount = document.createElement("div");
  const input = document.createElement("textarea");
  mount.append(input);
  document.body.append(mount);
  const send = vi.fn();
  const copy = vi.fn();
  const handler = new InputHandler(ghostty, mount, send, () => {});
  const unbindInput = bindTerminalTextInput(input, send);
  const unbindCopy = bindTerminalClipboard(
    mount,
    {
      getSelection: () => selection,
      hasSelection: () => selection.length > 0,
    },
    copy,
  );
  cleanups.push(() => {
    unbindCopy();
    unbindInput();
    handler.dispose();
    mount.remove();
  });
  const key = (init: KeyboardEventInit) => {
    const event = new KeyboardEvent("keydown", {
      key: "c",
      code: "KeyC",
      bubbles: true,
      cancelable: true,
      ...init,
    });
    input.dispatchEvent(event);
    return event;
  };
  return { mount, input, send, copy, key, unbindCopy };
}

describe("terminal clipboard with the installed Ghostty input handler", () => {
  it.each([
    { ctrlKey: true },
    { metaKey: true },
    { ctrlKey: true, shiftKey: true },
  ])("copies a canvas selection without sending PTY input: %j", (modifiers) => {
    const { key, send, copy } = setup();
    expect(key(modifiers).defaultPrevented).toBe(true);
    expect(copy).toHaveBeenCalledExactlyOnceWith("selected log");
    expect(send).not.toHaveBeenCalled();
  });
  it("keeps Ctrl+C as interrupt without a selection; copy-only shortcuts never interrupt", () => {
    const { key, send, copy } = setup("");
    key({ ctrlKey: true });
    key({ ctrlKey: true, shiftKey: true });
    key({ metaKey: true });
    expect(send.mock.calls).toEqual([["\u0003"]]);
    expect(copy.mock.calls).toEqual([[""], [""]]);
  });
  it("ignores IME composition and removes listeners on disposal", () => {
    const { input, key, copy, send, unbindCopy } = setup();
    input.dispatchEvent(new Event("compositionstart", { bubbles: true }));
    key({ ctrlKey: true });
    input.dispatchEvent(new Event("compositionend", { bubbles: true }));
    key({ metaKey: true, isComposing: true });
    expect(copy).not.toHaveBeenCalled();
    unbindCopy();
    key({ ctrlKey: true });
    expect(send).toHaveBeenCalledWith("\u0003");
  });
  it("writes native copy data synchronously and does not invoke another strategy", () => {
    const { input, copy } = setup();
    const event = new Event("copy", { bubbles: true, cancelable: true });
    const setData = vi.fn();
    Object.defineProperty(event, "clipboardData", { value: { setData } });
    input.dispatchEvent(event);
    expect(setData).toHaveBeenCalledExactlyOnceWith(
      "text/plain",
      "selected log",
    );
    expect(event.defaultPrevented).toBe(true);
    expect(copy).not.toHaveBeenCalled();
  });
  it("does not overwrite the clipboard with an empty selection or intercept outside inputs", () => {
    const { input, copy } = setup("");
    const event = new Event("copy", { bubbles: true, cancelable: true });
    const setData = vi.fn();
    Object.defineProperty(event, "clipboardData", { value: { setData } });
    input.dispatchEvent(event);
    expect(setData).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
    const outside = document.createElement("textarea");
    document.body.append(outside);
    const key = new KeyboardEvent("keydown", {
      key: "c",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    outside.dispatchEvent(key);
    outside.remove();
    expect(key.defaultPrevented).toBe(false);
    expect(copy).not.toHaveBeenCalled();
  });
});

describe("terminal buffer snapshot with real WASM", () => {
  const buffer = (cols = 32, rows = 3, scrollbackLimit = 100) => {
    const terminal = ghostty.createTerminal(cols, rows, { scrollbackLimit });
    // Match the application's clearTerminal before attaching a new session.
    terminal.write("\u001b[2J\u001b[3J\u001b[H");
    cleanups.push(() => terminal.free());
    return terminal;
  };
  it("includes retained history and screen, independently of viewport selection", () => {
    const terminal = buffer();
    const lines = Array.from({ length: 20 }, (_, i) => `log-${i}`);
    terminal.write(lines.join("\r\n"));
    expect(snapshotTerminalText(terminal)).toBe(lines.join("\n"));
  });
  it("preserves Chinese, emoji, combining characters, indentation and interior blank lines", () => {
    const terminal = buffer();
    terminal.write("  中文 👩‍💻 e\u0301  end\r\n\r\nlast\r\n");
    expect(snapshotTerminalText(terminal)).toBe(
      "  中文 👩‍💻 e\u0301  end\n\nlast",
    );
  });
  it("exports rendered long lines and only the active alternate screen", () => {
    const terminal = buffer(5, 3);
    terminal.write("abcdefgh\r\n");
    expect(snapshotTerminalText(terminal)).toBe("abcde\nfgh");
    terminal.write("\u001b[?1049h\u001b[H\u001b[2JALT");
    expect(snapshotTerminalText(terminal)).toBe("ALT");
    terminal.write("\u001b[?1049l");
    expect(snapshotTerminalText(terminal)).toBe("abcde\nfgh");
  });
  it("returns no padded rows for an empty terminal", () => {
    expect(snapshotTerminalText(buffer())).toBe("");
    expect(snapshotTerminalText(undefined)).toBe("");
  });
  it("never includes evicted history", () => {
    const terminal = buffer(20, 3, 3);
    terminal.write(
      Array.from({ length: 20000 }, (_, i) => `log-${i}`).join("\r\n"),
    );
    const text = snapshotTerminalText(terminal);
    expect(text).toContain("log-19999");
    expect(text.split("\n")).toHaveLength(
      terminal.getScrollbackLength() + terminal.rows,
    );
    expect(text.split("\n")).not.toContain("log-0");
  });
});
