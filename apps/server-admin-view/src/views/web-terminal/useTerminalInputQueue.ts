import type { Ref } from "vue";
import type { TerminalAttachmentRecord } from "@/types";
import {
  INPUT_BATCH_MAX_BYTES,
  INPUT_BATCH_WINDOW_MS,
} from "./terminal-runtime";
import {
  encodeInputToBase64,
  getInputByteLength,
  isSafeRemoteTerminalResponse,
  summarizeTerminalResponseCodePoints,
} from "./terminal-input";

type TerminalConnectionState = "idle" | "connecting" | "connected" | "error";

export const useTerminalInputQueue = ({
  activeAttachment,
  connectionError,
  connectionState,
  selectedSessionId,
  sendInput,
  translate,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  connectionError: Ref<string>;
  connectionState: Ref<TerminalConnectionState>;
  selectedSessionId: Ref<string>;
  sendInput: (attachmentId: string, payload: string) => Promise<unknown>;
  translate: (key: string) => string;
}) => {
  let inputFlushTimer: number | null = null;
  let pendingInputBuffer = "";
  let pendingInputBytes = 0;
  let inputSendQueue: Promise<unknown> = Promise.resolve();

  const shouldFlushInputImmediately = (data: string): boolean =>
    data.includes("\r") ||
    data.includes("\n") ||
    data.includes("\u0003") ||
    data.includes("\u0004") ||
    data.includes("\u001b") ||
    getInputByteLength(data) >= INPUT_BATCH_MAX_BYTES;

  const clearPendingInput = () => {
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
    console.error(error);
    connectionState.value = "error";
    connectionError.value =
      error instanceof Error
        ? error.message
        : translate("admin.webTerminal.inputSendFailed");
  };

  const queueInputPayload = (attachmentId: string, payload: string) => {
    inputSendQueue = inputSendQueue
      .catch(() => undefined)
      .then(async () => {
        if (activeAttachment.value?.id !== attachmentId) return;
        await sendInput(attachmentId, encodeInputToBase64(payload));
      })
      .catch((error) => {
        if (activeAttachment.value?.id !== attachmentId) return;
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
    if (!pendingInputBuffer) return;
    if (!attachmentId) {
      console.warn("[terminal] input flush deferred until attachment is ready", {
        connectionState: connectionState.value,
        bufferedBytes: pendingInputBytes,
        selectedSessionId: selectedSessionId.value || null,
      });
      return;
    }

    const payload = pendingInputBuffer;
    pendingInputBuffer = "";
    pendingInputBytes = 0;
    await queueInputPayload(attachmentId, payload);
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

    pendingInputBuffer += data;
    pendingInputBytes += getInputByteLength(data);

    if (
      options?.immediate ||
      shouldFlushInputImmediately(data) ||
      pendingInputBytes >= INPUT_BATCH_MAX_BYTES
    ) {
      void flushPendingInput();
      return;
    }

    scheduleInputFlush();
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
      await sendInput(attachmentId, encodeInputToBase64(payload));
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
