import type { Ref } from "vue";
import type { TerminalAttachmentRecord } from "@/lib/api/terminal";
import {
  INPUT_BATCH_MAX_BYTES,
  INPUT_BATCH_WINDOW_MS,
} from "./terminal-runtime";
import {
  encodeInputToBase64,
  getInputByteLength,
  isSafeRemoteTerminalResponse,
  splitTerminalInputByByteLength,
  summarizeTerminalResponseCodePoints,
} from "./terminal-input";

const MAX_PENDING_INPUT_BYTES = 64 * 1024;

type TerminalConnectionState = "idle" | "connecting" | "connected" | "error";

export const useTerminalInputQueue = ({
  activeAttachment,
  connectionState,
  onSendError,
  selectedSessionId,
  sendInput,
  translate,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  connectionState: Readonly<Ref<TerminalConnectionState>>;
  onSendError: (error: unknown) => void;
  selectedSessionId: Ref<string>;
  sendInput: (attachmentId: string, payload: string) => Promise<unknown>;
  translate: (key: string) => string;
}) => {
  let inputFlushTimer: number | null = null;
  let pendingInputBuffer = "";
  let pendingInputBytes = 0;
  let inputSendQueue: Promise<unknown> = Promise.resolve();
  let inputQueueGeneration = 0;

  const shouldFlushInputImmediately = (data: string): boolean =>
    data.includes("\r") ||
    data.includes("\n") ||
    data.includes("\u0003") ||
    data.includes("\u0004") ||
    data.includes("\u001b") ||
    getInputByteLength(data) >= INPUT_BATCH_MAX_BYTES;

  const clearPendingInput = () => {
    inputQueueGeneration += 1;
    if (inputFlushTimer) {
      window.clearTimeout(inputFlushTimer);
      inputFlushTimer = null;
    }
    pendingInputBuffer = "";
    pendingInputBytes = 0;
  };

  const getPendingInputSnapshot = () => ({
    byteLength: pendingInputBytes,
    hasPendingInput: pendingInputBuffer.length > 0,
  });

  const setInputSendError = (error: unknown) => {
    onSendError(error);
  };

  const queueInputPayload = (attachmentId: string, payload: string) => {
    const generation = inputQueueGeneration;
    inputSendQueue = inputSendQueue
      .catch(() => undefined)
      .then(async () => {
        if (
          generation !== inputQueueGeneration ||
          activeAttachment.value?.id !== attachmentId
        ) {
          return;
        }
        await sendInput(attachmentId, encodeInputToBase64(payload));
      })
      .catch((error) => {
        if (
          generation !== inputQueueGeneration ||
          activeAttachment.value?.id !== attachmentId
        ) {
          return;
        }
        setInputSendError(error);
      });
    return inputSendQueue;
  };

  const flushPendingInput = async () => {
    if (inputFlushTimer) {
      window.clearTimeout(inputFlushTimer);
      inputFlushTimer = null;
    }

    const attachmentId = activeAttachment.value?.id;
    if (!pendingInputBuffer) {
      await inputSendQueue;
      return;
    }
    if (!attachmentId) {
      console.warn(
        "[terminal] input flush deferred until attachment is ready",
        {
          connectionState: connectionState.value,
          bufferedBytes: pendingInputBytes,
          selectedSessionId: selectedSessionId.value || null,
        },
      );
      return;
    }

    const payloads = splitTerminalInputByByteLength(
      pendingInputBuffer,
      INPUT_BATCH_MAX_BYTES,
    );
    pendingInputBuffer = "";
    pendingInputBytes = 0;
    for (const payload of payloads) queueInputPayload(attachmentId, payload);
    await inputSendQueue;
  };

  const scheduleInputFlush = () => {
    if (inputFlushTimer) return;
    inputFlushTimer = window.setTimeout(() => {
      inputFlushTimer = null;
      void flushPendingInput();
    }, INPUT_BATCH_WINDOW_MS);
  };

  const queueTerminalInput = (
    data: string,
    options?: { immediate?: boolean },
  ) => {
    if (!data) {
      return;
    }

    if (!activeAttachment.value && connectionState.value !== "connecting") {
      console.warn("[terminal] dropping input without active attachment", {
        connectionState: connectionState.value,
        selectedSessionId: selectedSessionId.value || null,
        byteLength: getInputByteLength(data),
        immediate: options?.immediate === true,
      });
      return;
    }

    if (!activeAttachment.value && pendingInputBuffer.length === 0) {
      console.warn(
        "[terminal] buffering early input before attachment is ready",
        {
          connectionState: connectionState.value,
          selectedSessionId: selectedSessionId.value || null,
          byteLength: getInputByteLength(data),
          immediate: options?.immediate === true,
        },
      );
    }

    const combined = `${pendingInputBuffer}${data}`;
    const attachmentId = activeAttachment.value?.id;
    if (!attachmentId) {
      const bounded = splitTerminalInputByByteLength(
        combined,
        MAX_PENDING_INPUT_BYTES,
      );
      pendingInputBuffer = bounded[0] ?? "";
      pendingInputBytes = getInputByteLength(pendingInputBuffer);
      if (bounded.length > 1) {
        console.warn(
          "[terminal] dropped input beyond the pending buffer limit",
          {
            bufferedBytes: pendingInputBytes,
            connectionState: connectionState.value,
            selectedSessionId: selectedSessionId.value || null,
          },
        );
      }
      return;
    }

    if (inputFlushTimer) {
      window.clearTimeout(inputFlushTimer);
      inputFlushTimer = null;
    }
    pendingInputBuffer = "";
    pendingInputBytes = 0;
    const chunks = splitTerminalInputByByteLength(
      combined,
      INPUT_BATCH_MAX_BYTES,
    );
    const flushTail = options?.immediate || shouldFlushInputImmediately(data);
    chunks.forEach((chunk, index) => {
      const bytes = getInputByteLength(chunk);
      const isTail = index === chunks.length - 1;
      if (!isTail || flushTail || bytes >= INPUT_BATCH_MAX_BYTES) {
        queueInputPayload(attachmentId, chunk);
      } else {
        pendingInputBuffer = chunk;
        pendingInputBytes = bytes;
      }
    });
    if (pendingInputBuffer) scheduleInputFlush();
  };

  const queueRemoteTerminalResponse = (data: string) => {
    if (!isSafeRemoteTerminalResponse(data)) {
      console.warn("[terminal] dropped unexpected terminal response", {
        codePoints: summarizeTerminalResponseCodePoints(data),
        length: data.length,
      });
      return;
    }

    queueTerminalInput(data, { immediate: true });
  };

  const sendTerminalPayloadNow = async (payload: string) => {
    if (!payload) return;
    await flushPendingInput().catch(() => undefined);

    const attachmentId = activeAttachment.value?.id;
    if (!attachmentId) {
      throw new Error(translate("admin.webTerminal.noConnection"));
    }

    try {
      for (const chunk of splitTerminalInputByByteLength(
        payload,
        INPUT_BATCH_MAX_BYTES,
      )) {
        if (activeAttachment.value?.id !== attachmentId) {
          throw new Error(translate("admin.webTerminal.noConnection"));
        }
        await sendInput(attachmentId, encodeInputToBase64(chunk));
      }
    } catch (error) {
      setInputSendError(error);
      throw error;
    }
  };

  return {
    clearPendingInput,
    flushPendingInput,
    getPendingInputSnapshot,
    queueRemoteTerminalResponse,
    queueTerminalInput,
    sendTerminalPayloadNow,
  };
};
