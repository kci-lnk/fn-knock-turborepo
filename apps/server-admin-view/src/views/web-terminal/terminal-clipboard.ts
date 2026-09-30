/** Bridge the canvas selection to browser clipboard actions, never to PTY input. */
export function bindTerminalClipboard(
  mount: HTMLElement,
  terminal: { getSelection: () => string; hasSelection: () => boolean },
  copyText: (text: string) => void,
): () => void {
  let composing = false;
  const onCompositionStart = () => {
    composing = true;
  };
  const onCompositionEnd = () => {
    composing = false;
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (composing || event.isComposing || event.keyCode === 229 || event.altKey)
      return;
    if (event.code !== "KeyC" && event.key.toLowerCase() !== "c") return;
    const copyOnly = event.metaKey || (event.ctrlKey && event.shiftKey);
    if (!copyOnly && !(event.ctrlKey && terminal.hasSelection())) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    // Prevent the native copy event too: one gesture performs one explicit write.
    copyText(terminal.getSelection());
  };
  const onCopy = (event: ClipboardEvent) => {
    if (event.defaultPrevented) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const text = terminal.getSelection();
    if (!text.trim()) return;
    if (event.clipboardData) {
      event.clipboardData.setData("text/plain", text);
    } else {
      copyText(text);
    }
  };
  mount.addEventListener("keydown", onKeyDown, true);
  mount.addEventListener("copy", onCopy, true);
  mount.addEventListener("compositionstart", onCompositionStart, true);
  mount.addEventListener("compositionend", onCompositionEnd, true);
  mount.addEventListener("blur", onCompositionEnd, true);
  return () => {
    mount.removeEventListener("keydown", onKeyDown, true);
    mount.removeEventListener("copy", onCopy, true);
    mount.removeEventListener("compositionstart", onCompositionStart, true);
    mount.removeEventListener("compositionend", onCompositionEnd, true);
    mount.removeEventListener("blur", onCompositionEnd, true);
  };
}
