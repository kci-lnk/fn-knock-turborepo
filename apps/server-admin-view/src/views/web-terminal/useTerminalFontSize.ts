import { ref, type Ref } from "vue";
import { clampTerminalFontSize } from "./terminal-dom";
import {
  DEFAULT_TERMINAL_FONT_SIZE,
  DEFAULT_TERMINAL_FONT_SIZE_MOBILE,
  TERMINAL_FONT_SIZE_KEY,
} from "./terminal-runtime";

type TerminalFontSizeTarget = {
  options: {
    fontSize: number;
  };
};

export const useTerminalFontSize = ({
  compactViewport,
  getTerminal,
  scheduleFit,
}: {
  compactViewport: Ref<boolean>;
  getTerminal: () => TerminalFontSizeTarget | null;
  scheduleFit: () => void;
}) => {
  const terminalFontSize = ref(DEFAULT_TERMINAL_FONT_SIZE);

  const persistTerminalFontSize = () => {
    localStorage.setItem(
      TERMINAL_FONT_SIZE_KEY,
      String(terminalFontSize.value),
    );
  };

  const loadTerminalFontSize = () => {
    const stored = Number(localStorage.getItem(TERMINAL_FONT_SIZE_KEY) || "");
    if (Number.isFinite(stored)) {
      terminalFontSize.value = clampTerminalFontSize(stored);
      return;
    }
    terminalFontSize.value = compactViewport.value
      ? DEFAULT_TERMINAL_FONT_SIZE_MOBILE
      : DEFAULT_TERMINAL_FONT_SIZE;
  };

  const applyTerminalFontSize = (
    value: number,
    options?: { persist?: boolean },
  ) => {
    const nextFontSize = clampTerminalFontSize(value);
    if (nextFontSize === terminalFontSize.value) {
      if (options?.persist !== false) {
        persistTerminalFontSize();
      }
      return;
    }

    terminalFontSize.value = nextFontSize;
    if (options?.persist !== false) {
      persistTerminalFontSize();
    }

    const terminal = getTerminal();
    if (!terminal) return;
    terminal.options.fontSize = nextFontSize;
    scheduleFit();
  };

  const nudgeTerminalFontSize = (delta: number) => {
    applyTerminalFontSize(terminalFontSize.value + delta);
  };

  const resetTerminalFontSize = () => {
    applyTerminalFontSize(
      compactViewport.value
        ? DEFAULT_TERMINAL_FONT_SIZE_MOBILE
        : DEFAULT_TERMINAL_FONT_SIZE,
    );
  };

  return {
    applyTerminalFontSize,
    loadTerminalFontSize,
    nudgeTerminalFontSize,
    persistTerminalFontSize,
    resetTerminalFontSize,
    terminalFontSize,
  };
};
