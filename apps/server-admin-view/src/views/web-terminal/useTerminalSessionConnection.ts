import { watch, type Ref } from "vue";
import type { TerminalSessionRecord } from "@/lib/api/terminal";

export const useTerminalSessionConnection = ({
  attach,
  canInput,
  clearDetachedState,
  detachAttachment,
  ensureTerminalReady,
  flushPendingInput,
  markSyncedResize,
  scheduleResize,
  selectSession,
  stopControlQueues,
}: {
  attach: (session: TerminalSessionRecord) => Promise<void>;
  canInput: Ref<boolean>;
  clearDetachedState: () => void;
  detachAttachment: () => Promise<void>;
  ensureTerminalReady: () => Promise<void>;
  flushPendingInput: () => Promise<void>;
  markSyncedResize: (sessionId: string, cols: number, rows: number) => void;
  scheduleResize: () => void;
  selectSession: (sessionId: string) => void;
  stopControlQueues: () => void;
}) => {
  let disposed = false;
  let generation = 0;
  const stopInputWatch = watch(
    canInput,
    (enabled, previouslyEnabled) => {
      if (!enabled && previouslyEnabled) stopControlQueues();
    },
    { flush: "sync" },
  );

  const connect = async (session: TerminalSessionRecord) => {
    if (disposed) return;
    const operation = ++generation;
    selectSession(session.id);
    await ensureTerminalReady();
    if (operation !== generation) return;
    clearDetachedState();
    markSyncedResize(session.id, session.cols, session.rows);
    await attach(session);
    if (operation !== generation) return;
    scheduleResize();
    void flushPendingInput();
  };

  const detach = async () => {
    generation += 1;
    clearDetachedState();
    await detachAttachment();
  };

  const dispose = () => {
    disposed = true;
    generation += 1;
    stopInputWatch();
    clearDetachedState();
  };

  return { connect, detach, dispose };
};
