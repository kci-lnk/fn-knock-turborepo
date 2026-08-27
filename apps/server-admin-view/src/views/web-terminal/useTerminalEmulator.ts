import { nextTick, ref, type Ref } from "vue";
import type { TerminalOutputEvent } from "@/lib/api/terminal";
import {
  ensureGhostty,
  type ArmedModifier,
  type GhosttyModule,
} from "./terminal-runtime";
import { focusElementWithoutScroll } from "./terminal-dom";
import { decodeBase64ToBytes, encodeCtrlInput } from "./terminal-input";
import { createTerminalMouseReporter } from "./terminal-mouse";
import { createTerminalFitController } from "./terminal-fit";
import { createTerminalTouchGestures } from "./terminal-touch";

interface UseTerminalEmulatorOptions {
  applyFontSize: (value: number, options?: { persist?: boolean }) => void;
  canAcceptInput: () => boolean;
  compactViewport: Ref<boolean>;
  persistFontSize: () => void;
  queueInput: (payload: string, options?: { immediate?: boolean }) => void;
  queueRemoteResponse: (payload: string) => void;
  scheduleResize: () => void;
  terminalFontSize: Ref<number>;
  terminalFrameRef: Ref<HTMLElement | null>;
  translate: (key: string) => string;
}

export function useTerminalEmulator({
  applyFontSize,
  canAcceptInput,
  compactViewport,
  persistFontSize,
  queueInput,
  queueRemoteResponse,
  scheduleResize,
  terminalFontSize,
  terminalFrameRef,
  translate,
}: UseTerminalEmulatorOptions) {
  const terminalMountRef = ref<HTMLElement | null>(null);
  const isPinchZooming = ref(false);
  const armedModifier = ref<ArmedModifier | null>(null);
  let term: InstanceType<GhosttyModule["Terminal"]> | null = null;
  let fitAddon: InstanceType<GhosttyModule["FitAddon"]> | null = null;
  let lastOutputCursor = 0;
  let outputTextDecoder = new TextDecoder();
  let remoteOutputWriteDepth = 0;
  let terminalInternalResponseDropDepth = 0;
  let disposed = false;
  let initializationPromise: Promise<void> | null = null;

  function runTerminalInternalMutation(
    action: () => void,
    options?: { dropResponses?: boolean },
  ) {
    remoteOutputWriteDepth += 1;
    if (options?.dropResponses) terminalInternalResponseDropDepth += 1;
    try {
      action();
    } finally {
      if (options?.dropResponses) terminalInternalResponseDropDepth -= 1;
      remoteOutputWriteDepth -= 1;
    }
  }

  const fitController = createTerminalFitController({
    getFitAddon: () => fitAddon,
    getMountElement: () => terminalMountRef.value,
    getTerminal: () => term,
    runTerminalMutation: (mutation) =>
      runTerminalInternalMutation(mutation, { dropResponses: true }),
  });

  const getTerminalTextInput = (): HTMLTextAreaElement | null => {
    const input = terminalMountRef.value?.querySelector("textarea");
    return input instanceof HTMLTextAreaElement ? input : null;
  };

  const syncTerminalTextInputAnchor = () => {
    const textInput = getTerminalTextInput();
    if (!textInput) return;
    textInput.style.position = compactViewport.value ? "fixed" : "absolute";
    textInput.style.left = "0";
    textInput.style.top = "0";
    textInput.style.width = "1px";
    textInput.style.height = "1px";
    textInput.style.padding = "0";
    textInput.style.border = "none";
    textInput.style.margin = "0";
    textInput.style.opacity = "0";
    textInput.style.clipPath = "inset(50%)";
    textInput.style.overflow = "hidden";
    textInput.style.whiteSpace = "nowrap";
    textInput.style.resize = "none";
    textInput.style.pointerEvents = "none";
    textInput.style.fontSize = "16px";
  };

  const focusTerminal = () => {
    syncTerminalTextInputAnchor();
    if (compactViewport.value) {
      const textInput = getTerminalTextInput();
      if (textInput) {
        focusElementWithoutScroll(textInput);
        void nextTick(() => {
          const nextInput = getTerminalTextInput();
          if (nextInput) focusElementWithoutScroll(nextInput);
        });
        return;
      }
    }
    term?.focus();
    void nextTick(() => term?.focus());
  };

  const touchGestures = createTerminalTouchGestures({
    applyFontSize,
    compactViewport,
    getMountElement: () => terminalMountRef.value,
    getTerminal: () => term,
    isPinchZooming,
    persistFontSize,
    terminalFontSize,
  });
  const mouseReporter = createTerminalMouseReporter({
    focusTerminal,
    getFrameElement: () => terminalFrameRef.value,
    getMountElement: () => terminalMountRef.value,
    getRowHeight: () => touchGestures.getRowHeight(),
    getTerminal: () => term,
    queueInput: (payload) => queueInput(payload, { immediate: true }),
  });

  const resetOutputState = () => {
    lastOutputCursor = 0;
    outputTextDecoder = new TextDecoder();
  };
  const writeRemoteTerminalOutput = (payload: string) => {
    if (!term) return;
    remoteOutputWriteDepth += 1;
    try {
      term.write(payload);
    } finally {
      remoteOutputWriteDepth -= 1;
    }
  };
  const clearTerminal = () => {
    resetOutputState();
    if (!term) return;
    term.clear?.();
    term.reset();
    term.write("\u001b[2J\u001b[3J\u001b[H");
    focusTerminal();
  };
  const clearArmedModifier = () => {
    armedModifier.value = null;
  };
  const applyArmedModifierToInput = (value: string) => {
    const currentModifier = armedModifier.value;
    if (!currentModifier) return value;
    armedModifier.value = null;
    if (currentModifier === "alt") return `\u001b${value}`;
    return encodeCtrlInput(value) ?? value;
  };
  const toggleArmedModifier = (modifier: ArmedModifier) => {
    if (!canAcceptInput()) return;
    armedModifier.value = armedModifier.value === modifier ? null : modifier;
    focusTerminal();
  };

  const applyOutputEvent = (event: TerminalOutputEvent) => {
    if (!term) return;
    if (event.reset) {
      term.reset();
      outputTextDecoder = new TextDecoder();
      lastOutputCursor = 0;
    }
    if (event.dataBase64) {
      const payload = outputTextDecoder.decode(
        decodeBase64ToBytes(event.dataBase64),
        { stream: true },
      );
      if (payload) writeRemoteTerminalOutput(payload);
    }
    lastOutputCursor = event.cursor;
    void nextTick(() => focusTerminal());
  };

  const initializeTerminal = async () => {
    if (disposed || !terminalMountRef.value || term) return;
    if (initializationPromise) return initializationPromise;

    initializationPromise = (async () => {
      const { Terminal, FitAddon, ghostty } = await ensureGhostty();
      const mountElement = terminalMountRef.value;
      if (disposed || !mountElement || term) return;

      const nextTerm = new Terminal({
        ghostty,
        fontSize: terminalFontSize.value,
        cursorBlink: true,
        fontFamily:
          '"SFMono-Regular", "SF Mono", ui-monospace, Menlo, Monaco, Consolas, monospace',
        theme: {
          background: "#1c1c1e",
          foreground: "#ebeef2",
          cursor: "#f8fafc",
          black: "#141416",
          red: "#f87171",
          green: "#4ade80",
          yellow: "#facc15",
          blue: "#60a5fa",
          magenta: "#f472b6",
          cyan: "#22d3ee",
          white: "#e2e8f0",
          brightBlack: "#475569",
          brightRed: "#fb7185",
          brightGreen: "#86efac",
          brightYellow: "#fde047",
          brightBlue: "#93c5fd",
          brightMagenta: "#f9a8d4",
          brightCyan: "#67e8f9",
          brightWhite: "#f8fafc",
        },
      });
      const nextFitAddon = new FitAddon();
      nextTerm.loadAddon(nextFitAddon);
      nextTerm.open(mountElement);
      term = nextTerm;
      fitAddon = nextFitAddon;
      syncTerminalTextInputAnchor();
      mouseReporter.bind();
      touchGestures.bind();
      fitController.apply();
      fitController.observeMountSize();
      fitController.schedule();
      focusTerminal();
      nextTerm.onData((data) => {
        if (terminalInternalResponseDropDepth > 0) return;
        if (remoteOutputWriteDepth > 0) {
          queueRemoteResponse(data);
          return;
        }
        queueInput(applyArmedModifierToInput(data));
      });
      nextTerm.onResize(scheduleResize);
    })();

    try {
      await initializationPromise;
    } finally {
      initializationPromise = null;
    }
  };

  const ensureTerminalReady = async () => {
    if (term) return;
    await nextTick();
    await initializeTerminal();
    if (!term) throw new Error(translate("admin.webTerminal.notReady"));
  };

  const dispose = () => {
    disposed = true;
    mouseReporter.unbind();
    touchGestures.unbind();
    fitController.dispose();
    fitAddon?.dispose();
    term?.dispose();
    fitAddon = null;
    term = null;
  };

  return {
    applyOutputEvent,
    armedModifier,
    clearArmedModifier,
    clearTerminal,
    dispose,
    ensureTerminalReady,
    focusTerminal,
    getOutputCursor: () => lastOutputCursor,
    getTerminal: () => term,
    getTerminalSize: () => ({
      cols: term?.cols || 120,
      rows: term?.rows || 32,
    }),
    isPinchZooming,
    resetOutputState,
    scheduleFit: fitController.schedule,
    syncTerminalTextInputAnchor,
    terminalMountRef,
    toggleArmedModifier,
  };
}
