import { Ghostty, InputHandler } from "ghostty-web";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { bindTerminalTextInput } from "@/views/web-terminal/terminal-text-input";

let ghostty: Ghostty;
const cleanups: (() => void)[] = [];
beforeAll(async () => {
  ghostty = await Ghostty.load();
});
afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
});

function setup(bridge = true) {
  const mount = document.createElement("div");
  const input = document.createElement("textarea");
  mount.append(input);
  document.body.append(mount);
  const send = vi.fn();
  // Reproduce Terminal.open's blocker with the actual installed input handler.
  mount.addEventListener("beforeinput", (event) => event.preventDefault());
  const handler = new InputHandler(ghostty, mount, send, () => {});
  const unbind = bridge ? bindTerminalTextInput(input, send) : () => {};
  cleanups.push(() => {
    unbind();
    handler.dispose();
    mount.remove();
  });
  const edit = (
    inputType: string,
    data: string | null,
    value = data ?? "",
    isComposing = false,
    cancelable = true,
  ) => {
    const before = new InputEvent("beforeinput", {
      bubbles: true,
      cancelable,
      inputType,
      data,
      isComposing,
    });
    if (input.dispatchEvent(before)) {
      input.value = value;
      input.dispatchEvent(
        new InputEvent("input", {
          bubbles: true,
          inputType,
          data,
          isComposing,
        }),
      );
    }
    return before;
  };
  const key = (key: string, code = "", keyCode = 0) =>
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        bubbles: true,
        cancelable: true,
        key,
        code,
        keyCode,
      }),
    );
  const composition = (type: string, data = "") => {
    // happy-dom aliases CompositionEvent to Event and omits its data field.
    const event = new Event(type, { bubbles: true });
    Object.defineProperty(event, "data", { value: data });
    return input.dispatchEvent(event);
  };
  return { mount, input, send, edit, key, composition, unbind };
}

describe("terminal native text input with ghostty-web", () => {
  it("redirects Ghostty mount focus to the textarea until disposal", () => {
    const { mount, input, unbind } = setup();
    mount.focus();
    expect(document.activeElement).toBe(input);
    unbind();
    mount.focus();
    expect(document.activeElement).toBe(mount);
  });
  it("reproduces missing Android text and Enter without the bridge", () => {
    const { key, edit, send } = setup(false);
    key("Unidentified", "", 229);
    expect(edit("insertText", "a").defaultPrevented).toBe(true);
    key("Unidentified", "", 229);
    edit("insertLineBreak", null, "\n");
    expect(send).not.toHaveBeenCalled();
  });
  it("sends Android 229 edits, input-only text and soft Enter exactly once", () => {
    const { key, edit, input, send } = setup();
    key("Unidentified", "", 229);
    expect(edit("insertText", "a").defaultPrevented).toBe(false);
    edit("insertText", "😀");
    input.value = "b";
    input.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        data: "b",
        inputType: "insertText",
      }),
    );
    key("Unidentified", "", 229);
    edit("insertLineBreak", null, "\n");
    expect(send.mock.calls).toEqual([["a"], ["😀"], ["b"], ["\r"]]);
    expect(input.value).toBe("");
  });
  it.each([
    "insertLineBreak",
    "insertParagraph",
    "deleteContentBackward",
    "deleteContentForward",
  ])("handles %s on an empty textarea", (type) => {
    const { edit, send } = setup();
    const expected =
      type === "deleteContentBackward"
        ? "\u007f"
        : type === "deleteContentForward"
          ? "\u001b[3~"
          : "\r";
    expect(edit(type, null).defaultPrevented).toBe(true);
    expect(send.mock.calls).toEqual([[expected]]);
    edit(type, null, "", false, false);
    expect(send.mock.calls).toEqual([[expected], [expected]]);
  });
  it("commits Chinese once when final input follows compositionend", () => {
    const { composition, edit, key, send } = setup();
    composition("compositionstart");
    edit("insertCompositionText", "ni", "ni", true, false);
    composition("compositionupdate", "你");
    edit("insertCompositionText", "你", "你", true, false);
    key("Enter", "Enter");
    expect(send).not.toHaveBeenCalled();
    composition("compositionend", "你");
    edit("insertText", "你", "你");
    edit("insertText", "你", "你");
    key("Enter", "Enter");
    expect(send.mock.calls).toEqual([["你"], ["你"], ["\r"]]);
  });
  it("handles final input before compositionend and a following edit", () => {
    const { composition, edit, send } = setup();
    composition("compositionstart");
    edit("insertCompositionText", "한", "한", true, false);
    edit("insertFromComposition", "한", "한");
    expect(send).not.toHaveBeenCalled();
    composition("compositionend", "한");
    edit("insertText", "a", "한a");
    expect(send.mock.calls).toEqual([["한"], ["a"]]);
  });
  it("does not send canceled candidates", () => {
    const { composition, edit, send } = setup();
    composition("compositionstart");
    edit("insertCompositionText", "ni", "ni", true);
    composition("compositionend", "");
    edit("insertText", "a");
    expect(send.mock.calls).toEqual([["a"]]);
  });
  it("preserves hardware text, Enter, Backspace, arrows and Ctrl+C", () => {
    const { input, key, send } = setup();
    expect(key("a", "KeyA")).toBe(false);
    expect(key("Enter", "Enter")).toBe(false);
    key("Backspace", "Backspace");
    key("ArrowUp", "ArrowUp");
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        bubbles: true,
        cancelable: true,
        key: "c",
        code: "KeyC",
        ctrlKey: true,
      }),
    );
    expect(send.mock.calls).toEqual([
      ["a"],
      ["\r"],
      ["\u007f"],
      ["\u001b[A"],
      ["\u0003"],
    ]);
  });
  it("leaves paste to Ghostty and removes listeners on disposal", () => {
    const { input, send, edit, unbind } = setup();
    const paste = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(paste, "clipboardData", {
      value: { getData: () => "echo test" },
    });
    input.dispatchEvent(paste);
    expect(send.mock.calls).toEqual([["echo test"]]);
    unbind();
    expect(edit("insertText", "a").defaultPrevented).toBe(true);
    expect(send).toHaveBeenCalledTimes(1);
  });
});
