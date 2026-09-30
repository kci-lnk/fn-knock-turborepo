export type CopyTextResult = {
  verified: boolean;
  method: string;
};

export type CopyTextOptions = {
  strategy?: 'write-text-first';
  verify?: boolean;
  /** Stop a delayed fallback after the caller's session or UI has changed. */
  shouldContinue?: () => boolean;
};

const normalizeClipboardText = (value: string) =>
  value.replace(/\r\n/g, '\n').replace(/\r/g, '\n').normalize('NFC');

const verifyClipboardText = async (expectedText: string) => {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.readText) {
    return null;
  }

  try {
    const actualText = await navigator.clipboard.readText();
    return (
      normalizeClipboardText(actualText) ===
      normalizeClipboardText(expectedText)
    );
  } catch {
    return null;
  }
};

const copyWithExecCommand = (text: string) => {
  if (typeof document === 'undefined') {
    return false;
  }

  const activeElement = document.activeElement;
  const selection = document.getSelection();
  const selectedRanges: Range[] = [];

  if (selection) {
    for (let index = 0; index < selection.rangeCount; index += 1) {
      selectedRanges.push(selection.getRangeAt(index).cloneRange());
    }
  }

  const handleCopy = (event: ClipboardEvent) => {
    if (!event.clipboardData) return;
    event.clipboardData.setData('text/plain', text);
    event.preventDefault();
  };

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute('readonly', '');
  textarea.setAttribute('aria-hidden', 'true');
  textarea.style.position = 'fixed';
  textarea.style.top = '0';
  textarea.style.left = '0';
  textarea.style.width = '1px';
  textarea.style.height = '1px';
  textarea.style.padding = '0';
  textarea.style.border = '0';
  textarea.style.opacity = '0.01';
  textarea.style.pointerEvents = 'none';
  textarea.style.zIndex = '-1';

  // Stay inside a modal's focus trap when retrying from a text dialog.
  const container =
    activeElement instanceof Element
      ? (activeElement.closest('[role="dialog"]') ?? document.body)
      : document.body;
  container.appendChild(textarea);
  document.addEventListener('copy', handleCopy, true);

  try {
    try {
      textarea.focus({ preventScroll: true });
    } catch {
      textarea.focus();
    }
    textarea.select();
    textarea.setSelectionRange(0, textarea.value.length);

    return document.execCommand('copy');
  } finally {
    document.removeEventListener('copy', handleCopy, true);
    textarea.remove();

    if (selection) {
      selection.removeAllRanges();
      selectedRanges.forEach((range) => selection.addRange(range));
    }

    if (activeElement instanceof HTMLElement) {
      try {
        activeElement.focus({ preventScroll: true });
      } catch {
        activeElement.focus();
      }
    }
  }
};

const copyWithClipboardItem = async (text: string) => {
  if (
    typeof navigator === 'undefined' ||
    !navigator.clipboard?.write ||
    typeof ClipboardItem === 'undefined'
  ) {
    return false;
  }

  await navigator.clipboard.write([
    new ClipboardItem({
      'text/plain': new Blob([text], { type: 'text/plain' }),
    }),
  ]);

  return true;
};

const copyWithWriteText = async (text: string) => {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) {
    return false;
  }

  await navigator.clipboard.writeText(text);
  return true;
};

export async function copyTextToClipboard(
  text: string,
  options: CopyTextOptions = {},
): Promise<CopyTextResult> {
  const errors: string[] = [];
  let unverifiedResult: CopyTextResult | null = null;
  let verificationWasAvailable = false;

  const runStrategy = async (
    method: string,
    strategy: () => boolean | Promise<boolean>,
  ) => {
    try {
      if (options.shouldContinue && !options.shouldContinue()) return;
      const copied = await strategy();

      if (!copied) {
        errors.push(`${method}: unavailable`);
        return;
      }

      if (options.verify === false) return { verified: false, method };
      const verified = await verifyClipboardText(text);

      if (verified === true) {
        return { verified: true, method };
      }

      if (verified === false) {
        verificationWasAvailable = true;
        errors.push(`${method}: clipboard content mismatch`);
        return;
      }

      unverifiedResult ??= { verified: false, method };
    } catch (error) {
      errors.push(
        `${method}: ${error instanceof Error ? error.message : 'failed'}`,
      );
    }

    return undefined;
  };

  const strategies =
    options.strategy === 'write-text-first'
      ? ([
          ['clipboard.writeText', () => copyWithWriteText(text)],
          ['execCommand', () => copyWithExecCommand(text)],
        ] as const)
      : ([
          ['execCommand', () => copyWithExecCommand(text)],
          ['clipboard.write', () => copyWithClipboardItem(text)],
          ['clipboard.writeText', () => copyWithWriteText(text)],
        ] as const);
  for (const [method, strategy] of strategies) {
    const result = await runStrategy(method, strategy);

    if (result) {
      return result;
    }
  }

  if (unverifiedResult && !verificationWasAvailable) {
    return unverifiedResult;
  }

  throw new Error(errors.join('; ') || 'Copy failed');
}
