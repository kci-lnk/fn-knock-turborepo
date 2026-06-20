import type { Ref } from "vue";
import type {
  TerminalAttachmentRecord,
  TerminalSessionRecord,
} from "@/types";
import { RESIZE_BATCH_WINDOW_MS } from "./terminal-runtime";
import { buildTerminalSizeKey } from "./terminal-input";

type TerminalSizeSource = {
  cols: number;
  rows: number;
};

export const useTerminalResizeQueue = ({
  activeAttachment,
  getTerminal,
  resizeAttachment,
  restartPollingFromSnapshot,
  sessions,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  getTerminal: () => TerminalSizeSource | null;
  resizeAttachment: (
    attachmentId: string,
    cols: number,
    rows: number,
  ) => Promise<TerminalSessionRecord>;
  restartPollingFromSnapshot: (attachment: TerminalAttachmentRecord) => void;
  sessions: Ref<TerminalSessionRecord[]>;
}) => {
  let resizeTimer: number | null = null;
  let pendingResizeTarget: TerminalSizeSource | null = null;
  let lastSyncedResizeKey = "";
  let lastRequestedResizeKey = "";
  let resizeSendQueue: Promise<void> = Promise.resolve();

  const syncSessionDimensions = (
    sessionId: string,
    cols: number,
    rows: number,
  ) => {
    sessions.value = sessions.value.map((session) =>
      session.id === sessionId ? { ...session, cols, rows } : session,
    );
  };

  const markSyncedResize = (sessionId: string, cols: number, rows: number) => {
    lastSyncedResizeKey = buildTerminalSizeKey(cols, rows);
    lastRequestedResizeKey = lastSyncedResizeKey;
    syncSessionDimensions(sessionId, cols, rows);
  };

  const resetResizeState = () => {
    if (resizeTimer) {
      window.clearTimeout(resizeTimer);
      resizeTimer = null;
    }
    pendingResizeTarget = null;
    lastSyncedResizeKey = "";
    lastRequestedResizeKey = "";
    resizeSendQueue = Promise.resolve();
  };

  const flushPendingResize = async () => {
    if (resizeTimer) {
      window.clearTimeout(resizeTimer);
      resizeTimer = null;
    }

    const attachment = activeAttachment.value;
    const nextTarget = pendingResizeTarget;
    if (!attachment || !nextTarget) return;

    const resizeKey = buildTerminalSizeKey(nextTarget.cols, nextTarget.rows);
    if (
      resizeKey === lastSyncedResizeKey ||
      resizeKey === lastRequestedResizeKey
    ) {
      pendingResizeTarget = null;
      return;
    }

    pendingResizeTarget = null;
    lastRequestedResizeKey = resizeKey;

    resizeSendQueue = resizeSendQueue
      .catch(() => undefined)
      .then(async () => {
        if (activeAttachment.value?.id !== attachment.id) return;
        const session = await resizeAttachment(
          attachment.id,
          nextTarget.cols,
          nextTarget.rows,
        );
        if (activeAttachment.value?.id !== attachment.id) return;
        markSyncedResize(session.id, session.cols, session.rows);
        restartPollingFromSnapshot(attachment);
      })
      .catch((error) => {
        if (activeAttachment.value?.id !== attachment.id) return;
        console.error(error);
        lastRequestedResizeKey = lastSyncedResizeKey;
      });

    await resizeSendQueue;
  };

  const scheduleResize = () => {
    const terminal = getTerminal();
    if (!terminal || !activeAttachment.value) return;

    const nextTarget = { cols: terminal.cols, rows: terminal.rows };
    const resizeKey = buildTerminalSizeKey(nextTarget.cols, nextTarget.rows);
    if (
      resizeKey === lastSyncedResizeKey ||
      resizeKey === lastRequestedResizeKey
    ) {
      return;
    }

    pendingResizeTarget = nextTarget;
    if (resizeTimer) window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      void flushPendingResize();
    }, RESIZE_BATCH_WINDOW_MS);
  };

  return {
    flushPendingResize,
    markSyncedResize,
    resetResizeState,
    scheduleResize,
  };
};
