import { nextTick, type ComputedRef, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { TerminalAPI } from "@/lib/api/terminal";
import type {
  TerminalAttachmentRecord,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTransport,
} from "@/types";
import { RECENT_SESSION_KEY } from "./terminal-runtime";

type ConnectionState = "idle" | "connecting" | "connected" | "error";

export const useTerminalSessionController = ({
  activeAttachment,
  activeTransport,
  applyOutputChunk,
  clearArmedModifier,
  clearPendingInput,
  clearTerminal,
  connectionError,
  connectionState,
  ensureConfig,
  ensureTerminalReady,
  flushPendingInput,
  flushPendingResize,
  getOutputCursor,
  getPendingInputSnapshot,
  getTerminalSize,
  isBooting,
  isCreating,
  isKilling,
  markSyncedResize,
  onBootstrapLayoutReady,
  resetOutputState,
  resetResizeState,
  runtimeStatus,
  scheduleResize,
  selectedSession,
  selectedSessionId,
  sessions,
  terminalEnabled,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  activeTransport: Ref<TerminalTransport | null>;
  applyOutputChunk: (chunk: TerminalOutputChunk) => void;
  clearArmedModifier: () => void;
  clearPendingInput: () => void;
  clearTerminal: () => void;
  connectionError: Ref<string>;
  connectionState: Ref<ConnectionState>;
  ensureConfig: () => Promise<void>;
  ensureTerminalReady: () => Promise<void>;
  flushPendingInput: () => Promise<void>;
  flushPendingResize: () => Promise<void>;
  getOutputCursor: () => number;
  getPendingInputSnapshot: () => {
    byteLength: number;
    hasPendingInput: boolean;
  };
  getTerminalSize: () => { cols: number; rows: number };
  isBooting: Ref<boolean>;
  isCreating: Ref<boolean>;
  isKilling: Ref<boolean>;
  markSyncedResize: (sessionId: string, cols: number, rows: number) => void;
  onBootstrapLayoutReady: () => Promise<void> | void;
  resetOutputState: () => void;
  resetResizeState: () => void;
  runtimeStatus: Ref<TerminalRuntimeStatus | null>;
  scheduleResize: () => void;
  selectedSession: ComputedRef<TerminalSessionRecord | null>;
  selectedSessionId: Ref<string>;
  sessions: Ref<TerminalSessionRecord[]>;
  terminalEnabled: Readonly<Ref<boolean>>;
}) => {
  const { t } = useI18n();
  let pollGeneration = 0;
  let disposed = false;

  const rememberRecentSession = (sessionId: string) => {
    try {
      localStorage.setItem(RECENT_SESSION_KEY, sessionId);
    } catch {
      // Recent-session restoration is optional.
    }
  };

  const refreshSessions = async () => {
    sessions.value = await TerminalAPI.listSessions();
    if (
      selectedSessionId.value &&
      !sessions.value.some((item) => item.id === selectedSessionId.value)
    ) {
      selectedSessionId.value = "";
    }
  };

  const stopCurrentConnection = async (detach = true) => {
    pollGeneration += 1;
    await flushPendingInput().catch(() => undefined);
    await flushPendingResize().catch(() => undefined);
    clearPendingInput();
    clearArmedModifier();
    resetResizeState();

    const attachmentId = activeAttachment.value?.id;
    activeAttachment.value = null;
    activeTransport.value = null;
    connectionState.value = "idle";
    connectionError.value = "";
    resetOutputState();

    if (detach && attachmentId) {
      await TerminalAPI.detachAttachment(attachmentId).catch(() => undefined);
    }
  };

  const startHttpPolling = async (attachment: TerminalAttachmentRecord) => {
    if (disposed) return;
    const generation = ++pollGeneration;
    connectionState.value = "connected";
    activeTransport.value = "http-polling";

    while (
      generation === pollGeneration &&
      activeAttachment.value?.id === attachment.id
    ) {
      try {
        const result = await TerminalAPI.pollAttachment(attachment.id, {
          cursor: getOutputCursor(),
          timeout_ms: 4500,
        });
        if (
          generation !== pollGeneration ||
          activeAttachment.value?.id !== attachment.id
        ) {
          return;
        }
        if (result.changed && result.chunk) {
          applyOutputChunk(result.chunk);
        }
      } catch (error) {
        if (generation !== pollGeneration) return;
        connectionState.value = "error";
        connectionError.value =
          error instanceof Error
            ? error.message
            : t("admin.webTerminal.pollingDisconnected");
        return;
      }
    }
  };

  const restartHttpPollingFromSnapshot = (
    attachment: TerminalAttachmentRecord,
  ) => {
    if (disposed || activeAttachment.value?.id !== attachment.id) return;
    resetOutputState();
    void startHttpPolling(attachment);
  };

  const connectToSession = async (session: TerminalSessionRecord) => {
    if (disposed) return;
    selectedSessionId.value = session.id;
    await ensureTerminalReady();
    if (disposed) return;
    await stopCurrentConnection();
    if (disposed) return;
    selectedSessionId.value = session.id;
    rememberRecentSession(session.id);

    connectionState.value = "connecting";
    connectionError.value = "";
    markSyncedResize(session.id, session.cols, session.rows);
    clearTerminal();

    let attachment: TerminalAttachmentRecord;
    try {
      attachment = await TerminalAPI.createAttachment(session.id);
    } catch (error) {
      const pendingInput = getPendingInputSnapshot();
      if (pendingInput.hasPendingInput) {
        console.warn(
          "[terminal] clearing buffered input after attachment failed",
          {
            sessionId: session.id,
            bufferedBytes: pendingInput.byteLength,
          },
        );
      }
      clearPendingInput();
      throw error;
    }

    if (disposed) {
      await TerminalAPI.detachAttachment(attachment.id).catch(() => undefined);
      return;
    }

    activeAttachment.value = attachment;
    const pendingInput = getPendingInputSnapshot();
    if (pendingInput.hasPendingInput) {
      console.warn("[terminal] attachment ready, flushing buffered input", {
        sessionId: session.id,
        attachmentId: attachment.id,
        bufferedBytes: pendingInput.byteLength,
      });
    }
    scheduleResize();
    void startHttpPolling(attachment);
    void flushPendingInput();
  };

  const handleSessionTabChange = async (sessionId: string | number) => {
    const nextSessionId = String(sessionId || "");
    if (!nextSessionId || nextSessionId === selectedSessionId.value) return;
    const nextSession =
      sessions.value.find((session) => session.id === nextSessionId) || null;
    if (!nextSession) return;

    try {
      await connectToSession(nextSession);
    } catch (error) {
      toast.error(t("admin.webTerminal.switchFailed"), {
        description:
          error instanceof Error
            ? error.message
            : t("admin.webTerminal.switchFailedDescription"),
      });
    }
  };

  const createSession = async (
    options: { toastOnSuccess?: boolean; connect?: boolean } = {},
  ): Promise<TerminalSessionRecord | null> => {
    const { toastOnSuccess = true, connect = true } = options;
    isCreating.value = true;
    try {
      const session = await TerminalAPI.createSession(getTerminalSize());
      await refreshSessions();
      if (connect) await connectToSession(session);
      if (toastOnSuccess) {
        toast.success(t("admin.webTerminal.sessionCreated"));
      }
      return session;
    } catch (error) {
      toast.error(t("admin.webTerminal.createFailed"), {
        description:
          error instanceof Error
            ? error.message
            : t("admin.webTerminal.createFailedDescription"),
      });
      return null;
    } finally {
      isCreating.value = false;
    }
  };

  const reconnectSession = async () => {
    if (!selectedSession.value) return;
    try {
      await connectToSession(selectedSession.value);
    } catch (error) {
      toast.error(t("admin.webTerminal.reconnectFailed"), {
        description:
          error instanceof Error
            ? error.message
            : t("admin.webTerminal.reconnectFailedDescription"),
      });
    }
  };

  const destroySelectedSession = async () => {
    if (!selectedSession.value) return;
    isKilling.value = true;
    try {
      await stopCurrentConnection();
      await TerminalAPI.deleteSession(selectedSession.value.id);
      await refreshSessions();
      const nextSession = sessions.value[0];
      if (nextSession) {
        await connectToSession(nextSession);
      } else {
        selectedSessionId.value = "";
        clearTerminal();
      }
      toast.success(t("admin.webTerminal.sessionEnded"));
    } catch (error) {
      toast.error(t("admin.webTerminal.endFailed"), {
        description:
          error instanceof Error
            ? error.message
            : t("admin.webTerminal.endFailedDescription"),
      });
    } finally {
      isKilling.value = false;
    }
  };

  const ensureDefaultSessionOnEntry = async (
    status: TerminalRuntimeStatus,
    sessionList: TerminalSessionRecord[],
  ) => {
    if (
      sessionList.length > 0 ||
      !terminalEnabled.value ||
      !status.enabled ||
      status.blockedReason
    ) {
      return sessionList;
    }
    const session = await createSession({
      toastOnSuccess: false,
      connect: false,
    });
    if (!session) return sessionList;
    return sessions.value.length > 0 ? sessions.value : [session];
  };

  const readRecentSessionId = () => {
    try {
      return localStorage.getItem(RECENT_SESSION_KEY) || "";
    } catch {
      return "";
    }
  };

  const bootstrapPage = async () => {
    let initialSession: TerminalSessionRecord | null = null;
    try {
      await ensureConfig();
      if (disposed) return;
      const [status, sessionList] = await Promise.all([
        TerminalAPI.getStatus(),
        TerminalAPI.listSessions(),
      ]);
      if (disposed) return;
      runtimeStatus.value = status;
      const resolvedSessions = await ensureDefaultSessionOnEntry(
        status,
        sessionList,
      );
      sessions.value = resolvedSessions;

      const remembered = readRecentSessionId();
      const firstSession =
        resolvedSessions.find((item) => item.id === remembered) ||
        resolvedSessions[0];
      if (firstSession && terminalEnabled.value && !status.blockedReason) {
        selectedSessionId.value = firstSession.id;
        initialSession = firstSession;
      }
    } catch (error) {
      connectionState.value = "error";
      connectionError.value =
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.initFailed");
    } finally {
      if (!disposed) {
        isBooting.value = false;
        await nextTick();
        await onBootstrapLayoutReady();
      }
    }

    if (initialSession && !disposed) {
      try {
        await connectToSession(initialSession);
      } catch (error) {
        connectionState.value = "error";
        connectionError.value =
          error instanceof Error
            ? error.message
            : t("admin.webTerminal.initFailed");
      }
    }
  };

  const dispose = async () => {
    disposed = true;
    await stopCurrentConnection();
  };

  return {
    bootstrapPage,
    connectToSession,
    createSession,
    dispose,
    destroySelectedSession,
    handleSessionTabChange,
    reconnectSession,
    refreshSessions,
    restartHttpPollingFromSnapshot,
    stopCurrentConnection,
  };
};
