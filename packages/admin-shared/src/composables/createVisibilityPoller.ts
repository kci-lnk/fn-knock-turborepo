export interface VisibilityPollerOptions {
  intervalMs: number;
  task: (signal: AbortSignal) => Promise<void> | void;
  enabled?: () => boolean;
  immediate?: boolean;
}

export const createVisibilityPoller = (options: VisibilityPollerOptions) => {
  let running = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let controller: AbortController | null = null;
  let inFlight: Promise<void> | null = null;
  let rerun = false;
  let generation = 0;

  const isVisible = () => typeof document === "undefined" || !document.hidden;
  const isEnabled = () => options.enabled?.() ?? true;
  const canRun = () => running && isVisible() && isEnabled();

  const clearTimer = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };

  const pauseCycle = () => {
    generation += 1;
    clearTimer();
    controller?.abort();
    controller = null;
    rerun = false;
  };

  const schedule = (delay: number) => {
    clearTimer();
    if (!canRun()) return;
    timer = setTimeout(() => {
      timer = null;
      void execute().catch(() => {
        // Poll tasks own their user-facing error handling. Keep a failed cycle
        // from becoming an unhandled rejection; the finally block reschedules.
      });
    }, delay);
  };

  const execute = async () => {
    if (!canRun()) return;
    if (inFlight) {
      rerun = true;
      return inFlight;
    }
    const token = generation;
    const activeController = new AbortController();
    controller = activeController;
    inFlight = Promise.resolve().then(() =>
      options.task(activeController.signal),
    );
    try {
      await inFlight;
    } finally {
      inFlight = null;
      if (controller === activeController) controller = null;
      if (canRun()) {
        if (token !== generation || rerun) {
          rerun = false;
          schedule(0);
        } else {
          schedule(options.intervalMs);
        }
      }
    }
  };

  const handleVisibilityChange = () => {
    if (!isVisible()) {
      pauseCycle();
      return;
    }
    if (running && isEnabled()) schedule(0);
  };

  const start = () => {
    if (running) return;
    running = true;
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", handleVisibilityChange);
    }
    schedule(options.immediate === false ? options.intervalMs : 0);
  };

  const stop = () => {
    if (!running) return;
    running = false;
    pauseCycle();
    if (typeof document !== "undefined") {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    }
  };

  const sync = () => {
    // `sync` is used when route parameters or enablement change. Abort the
    // obsolete request before scheduling the replacement so stale responses
    // cannot win after a fast selection change.
    pauseCycle();
    if (canRun()) schedule(0);
  };

  const refresh = () => {
    if (!running) return Promise.resolve();
    if (inFlight) {
      rerun = true;
      return inFlight;
    }
    clearTimer();
    return execute();
  };

  return { refresh, start, stop, sync };
};
