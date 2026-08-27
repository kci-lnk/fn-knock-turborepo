import type { Ref } from "vue";
import type { TerminalAttachmentRecord } from "@/lib/api/terminal";
import { RESIZE_BATCH_WINDOW_MS } from "./terminal-runtime";
import { buildTerminalSizeKey } from "./terminal-input";
import { normalizeTerminalDimensions } from "./terminal-dimensions";

type TerminalSizeSource = {
  cols: number;
  rows: number;
};

export const useTerminalResizeQueue = ({
  activeAttachment,
  getTerminal,
  onResizeSynced,
  resizeAttachment,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  getTerminal: () => TerminalSizeSource | null;
  resizeAttachment: (
    attachmentId: string,
    cols: number,
    rows: number,
  ) => Promise<void>;
  onResizeSynced: (sessionId: string, cols: number, rows: number) => void;
}) => {
  let resizeTimer: number | null = null;
  let pendingResizeTarget: TerminalSizeSource | null = null;
  let lastSyncedResizeKey = "";
  let lastRequestedResizeKey = "";
  let resizeSendQueue: Promise<void> = Promise.resolve();

  const markSyncedResize = (sessionId: string, cols: number, rows: number) => {
    lastSyncedResizeKey = buildTerminalSizeKey(cols, rows);
    lastRequestedResizeKey = lastSyncedResizeKey;
    onResizeSynced(sessionId, cols, rows);
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
        await resizeAttachment(attachment.id, nextTarget.cols, nextTarget.rows);
        if (activeAttachment.value?.id !== attachment.id) return;
        markSyncedResize(
          attachment.sessionId,
          nextTarget.cols,
          nextTarget.rows,
        );
      })
      .catch(() => {
        if (activeAttachment.value?.id !== attachment.id) return;
        lastRequestedResizeKey = lastSyncedResizeKey;
      });

    await resizeSendQueue;
  };

  const scheduleResize = () => {
    const terminal = getTerminal();
    if (!terminal || !activeAttachment.value) return;

    const nextTarget = normalizeTerminalDimensions(terminal);
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
