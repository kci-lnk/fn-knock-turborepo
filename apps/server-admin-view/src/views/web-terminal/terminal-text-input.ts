import { focusElementWithoutScroll } from "./terminal-dom";

/**
 * ghostty-web 0.4 handles keydown/composition, but cancels all beforeinput
 * events on its mount. Android keyboards often use keyCode 229 followed by
 * native textarea edits instead. Keep those edits local to the textarea and
 * forward committed text through the same input queue as hardware keys.
 */
export function bindTerminalTextInput(
  input: HTMLTextAreaElement,
  sendInput: (value: string) => void,
): () => void {
  let composing = false;
  let committedValue = "";
  const mount = input.parentElement;
  // Ghostty's open()/focus() focus the contenteditable mount again in a timer.
  // Native edits must reach the textarea even after that deferred focus.
  const onMountFocus = () => focusElementWithoutScroll(input);

  const send = (value: string) => {
    if (value) sendInput(value.replace(/\r\n|\n/g, "\r"));
  };
  const reset = () => {
    input.value = "";
    committedValue = "";
  };
  const controlInput = (type: string): string | null => {
    if (type === "insertLineBreak" || type === "insertParagraph") return "\r";
    if (type === "deleteContentBackward") return "\u007f";
    if (type === "deleteContentForward") return "\u001b[3~";
    return null;
  };
  const onBeforeInput = (event: InputEvent) => {
    // Stop Ghostty's bubbling preventDefault, but let the browser edit the
    // textarea (including non-cancelable IME edits).
    event.stopPropagation();
    const control = controlInput(event.inputType);
    // Backspace on an empty textarea may produce no input event at all.
    if (control && event.cancelable && !composing && !event.isComposing) {
      event.preventDefault();
      send(control);
      reset();
    }
  };
  const onInput = (event: Event) => {
    event.stopPropagation();
    if (!(event instanceof InputEvent)) return;
    if (composing || event.isComposing) return;

    const control = controlInput(event.inputType);
    if (control) {
      send(control);
    } else {
      // compositionend may precede the final input event. Retain the native
      // value until that event, so the already-sent composition is subtracted
      // rather than sent twice. This also works when the next edit appends text.
      const value = input.value;
      send(
        committedValue && value.startsWith(committedValue)
          ? value.slice(committedValue.length)
          : value || event.data || "",
      );
    }
    reset();
  };
  const onCompositionStart = (event: CompositionEvent) => {
    event.stopPropagation();
    composing = true;
    committedValue = "";
  };
  const onCompositionUpdate = (event: CompositionEvent) => {
    event.stopPropagation();
  };
  const onCompositionEnd = (event: CompositionEvent) => {
    event.stopPropagation();
    composing = false;
    send(event.data);
    committedValue = input.value;
    if (!event.data) reset();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    // Hardware keys still use Ghostty's escape-sequence encoder. During IME
    // composition, Enter selects a candidate and must not execute a command.
    if (composing || event.isComposing || event.keyCode === 229) {
      event.stopPropagation();
    }
  };
  const onBlur = () => {
    composing = false;
    reset();
  };

  input.addEventListener("beforeinput", onBeforeInput);
  input.addEventListener("input", onInput);
  input.addEventListener("compositionstart", onCompositionStart);
  input.addEventListener("compositionupdate", onCompositionUpdate);
  input.addEventListener("compositionend", onCompositionEnd);
  input.addEventListener("keydown", onKeyDown);
  input.addEventListener("blur", onBlur);
  mount?.addEventListener("focus", onMountFocus);

  return () => {
    input.removeEventListener("beforeinput", onBeforeInput);
    input.removeEventListener("input", onInput);
    input.removeEventListener("compositionstart", onCompositionStart);
    input.removeEventListener("compositionupdate", onCompositionUpdate);
    input.removeEventListener("compositionend", onCompositionEnd);
    input.removeEventListener("keydown", onKeyDown);
    input.removeEventListener("blur", onBlur);
    mount?.removeEventListener("focus", onMountFocus);
    reset();
  };
}
