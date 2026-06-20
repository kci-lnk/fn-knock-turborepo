import { nextTick } from "vue";

type TerminalFitDimensions = {
  cols: number;
  rows: number;
};

type TerminalFitAddon = {
  proposeDimensions: () => TerminalFitDimensions | undefined;
};

type TerminalFitInstance = TerminalFitDimensions & {
  resize: (cols: number, rows: number) => void;
};

export const createTerminalFitController = ({
  getFitAddon,
  getMountElement,
  getTerminal,
  runTerminalMutation,
}: {
  getFitAddon: () => TerminalFitAddon | null;
  getMountElement: () => HTMLElement | null;
  getTerminal: () => TerminalFitInstance | null;
  runTerminalMutation: (mutation: () => void) => void;
}) => {
  let fitFrame: number | null = null;
  let fitTimer: number | null = null;
  let attemptsRemaining = 0;
  let resizeObserver: ResizeObserver | null = null;
  let observedElement: HTMLElement | null = null;

  const apply = () => {
    const terminal = getTerminal();
    const fitAddon = getFitAddon();
    if (!terminal || !fitAddon) return;

    const dimensions = fitAddon.proposeDimensions();
    if (!dimensions) return;
    if (dimensions.cols === terminal.cols && dimensions.rows === terminal.rows) {
      return;
    }

    runTerminalMutation(() => {
      getTerminal()?.resize(dimensions.cols, dimensions.rows);
    });
  };

  const hasCanvasHeightGap = (): boolean => {
    const mountElement = getMountElement();
    if (!mountElement) return false;

    const canvas = mountElement.querySelector("canvas");
    if (!(canvas instanceof HTMLCanvasElement)) return false;

    const mountHeight = mountElement.clientHeight;
    const canvasHeight = Math.round(canvas.getBoundingClientRect().height);
    if (mountHeight <= 0 || canvasHeight <= 0) return false;

    return Math.abs(mountHeight - canvasHeight) > 24;
  };

  const runFitAttempt = () => {
    apply();

    if (
      attemptsRemaining <= 0 ||
      !hasCanvasHeightGap() ||
      typeof window === "undefined"
    ) {
      attemptsRemaining = 0;
      return;
    }

    attemptsRemaining -= 1;
    fitTimer = window.setTimeout(() => {
      fitTimer = null;
      runFitAttempt();
    }, 120);
  };

  const schedule = () => {
    if (typeof window === "undefined") return;

    if (fitFrame !== null) {
      window.cancelAnimationFrame(fitFrame);
    }
    if (fitTimer !== null) {
      window.clearTimeout(fitTimer);
      fitTimer = null;
    }
    attemptsRemaining = 8;

    void nextTick(() => {
      fitFrame = window.requestAnimationFrame(() => {
        fitFrame = null;
        runFitAttempt();
      });
    });
  };

  const observeMountSize = () => {
    const mountElement = getMountElement();
    if (typeof ResizeObserver === "undefined" || !mountElement) {
      return;
    }
    if (resizeObserver && observedElement === mountElement) {
      return;
    }

    resizeObserver?.disconnect();
    resizeObserver = new ResizeObserver(() => {
      schedule();
    });
    resizeObserver.observe(mountElement);
    observedElement = mountElement;
  };

  const dispose = () => {
    if (typeof window !== "undefined") {
      if (fitFrame !== null) {
        window.cancelAnimationFrame(fitFrame);
      }
      if (fitTimer !== null) {
        window.clearTimeout(fitTimer);
      }
    }
    fitFrame = null;
    fitTimer = null;
    attemptsRemaining = 0;
    resizeObserver?.disconnect();
    resizeObserver = null;
    observedElement = null;
  };

  return {
    apply,
    dispose,
    observeMountSize,
    schedule,
  };
};
