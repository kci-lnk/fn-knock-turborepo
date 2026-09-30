import { TerminalAPI } from "@/lib/api/terminal";

// A page owns every session it has opened, even while another tab/target is
// selected inside the terminal. Output attachment lifetimes are independent.
export const useTerminalPageLease = ({
  onSuspend,
  onResume,
}: {
  onSuspend: () => void;
  onResume: (isCurrent: () => boolean) => Promise<void>;
}) => {
  let pageId: string | null = null;
  let pending: Promise<string> | null = null;
  let generation = 0;
  let active = false;
  let disposed = false;
  let timer: ReturnType<typeof setInterval> | undefined;
  let renewing: number | null = null;
  let resuming: { generation: number; promise: Promise<void> } | null = null;
  let resumeNeeded = false;

  const release = () => {
    generation += 1;
    pending = null;
    renewing = null;
    resuming = null;
    const previous = pageId;
    pageId = null;
    if (previous) void TerminalAPI.releasePage(previous).catch(() => undefined);
  };

  const ensurePageId = (): Promise<string> => {
    if (!active || disposed)
      return Promise.reject(new DOMException("Aborted", "AbortError"));
    if (pageId) return Promise.resolve(pageId);
    if (pending) return pending;
    const operation = generation;
    pending = TerminalAPI.registerPage()
      .then((page) => {
        if (operation !== generation || !active || disposed) {
          void TerminalAPI.releasePage(page.id).catch(() => undefined);
          throw new DOMException("Aborted", "AbortError");
        }
        pageId = page.id;
        return page.id;
      })
      .finally(() => {
        if (operation === generation) pending = null;
      });
    return pending;
  };

  const resume = (): Promise<void> => {
    if (!resumeNeeded || !active || disposed) return Promise.resolve();
    if (resuming?.generation === generation) return resuming.promise;
    const operation = generation;
    const promise = Promise.resolve()
      .then(() => {
        const isCurrent = () => operation === generation && active && !disposed;
        if (isCurrent()) return onResume(isCurrent);
      })
      .then(() => {
        if (operation === generation) resumeNeeded = false;
      })
      .finally(() => {
        if (resuming?.generation === operation) resuming = null;
      });
    resuming = { generation: operation, promise };
    return promise;
  };

  const renew = async () => {
    if (!active || disposed || renewing === generation) return;
    const operation = generation;
    renewing = operation;
    try {
      const id = await ensurePageId();
      if (operation !== generation || !active || disposed) return;
      try {
        await TerminalAPI.heartbeatPage(id);
      } catch (error) {
        if (operation !== generation || !active || disposed) return;
        // Only the heartbeat endpoint's 409 means this lease has expired.
        // A session resume can also fail with 409 (e.g. attachment limit),
        // which must never release a healthy page's other terminal processes.
        if (
          (error as { response?: { status?: number } }).response?.status !== 409
        )
          return;
        release();
        onSuspend();
        resumeNeeded = true;
        await ensurePageId();
        await resume();
        return;
      }
      if (operation === generation) await resume();
    } finally {
      // A request from a departed page must not block or clear the next
      // page's heartbeat, even if the old network request has not settled.
      if (renewing === operation) renewing = null;
    }
  };

  const hide = () => {
    active = false;
    resumeNeeded = true;
    release();
    onSuspend();
  };
  const show = (event: PageTransitionEvent) => {
    if (!event.persisted || disposed) return;
    active = true;
    const operation = generation;
    void ensurePageId()
      .then(() => {
        if (operation === generation && active && !disposed) return resume();
      })
      .catch(() => undefined);
  };
  const start = () => {
    if (disposed || timer !== undefined) return;
    active = true;
    window.addEventListener("pagehide", hide);
    window.addEventListener("pageshow", show);
    timer = setInterval(() => {
      void renew().catch(() => undefined);
    }, 30_000);
  };
  const dispose = () => {
    disposed = true;
    active = false;
    if (timer !== undefined) clearInterval(timer);
    window.removeEventListener("pagehide", hide);
    window.removeEventListener("pageshow", show);
    release();
  };

  return { start, dispose, ensurePageId, invalidate: release };
};
