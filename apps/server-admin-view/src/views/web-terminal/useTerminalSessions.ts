import { computed, ref, type Ref } from "vue";
import {
  TerminalAPI,
  type TerminalErrorCode,
  type TerminalSessionPhase,
  type TerminalSessionRecord,
} from "@/lib/api/terminal";
import { extractTerminalError } from "./terminal-errors";

const RECENT_SESSION_KEY = "fn-knock:terminal:last-session";
const terminalPhases = new Set(["closed", "exited", "lost", "failed"]);

const readRecentSession = () => {
  try {
    return localStorage.getItem(RECENT_SESSION_KEY) ?? "";
  } catch {
    return "";
  }
};

type OperationSlot = {
  controller: AbortController | null;
  generation: number;
};

const newOperationSlot = (): OperationSlot => ({
  controller: null,
  generation: 0,
});

export const useTerminalSessions = ({
  selectedTargetId,
  onRuntimeChanged,
}: {
  selectedTargetId: Ref<string>;
  onRuntimeChanged?: (previousRuntimeId: string, runtimeId: string) => void;
}) => {
  const sessions = ref<TerminalSessionRecord[]>([]);
  const selectedSessionId = ref("");
  const runtimeId = ref("");
  const loading = ref(false);
  const creating = ref(false);
  const ending = ref(false);
  const error = ref("");
  const errorCode = ref<TerminalErrorCode | null>(null);
  let loadGeneration = 0;
  let loadController: AbortController | null = null;
  const createOperation = newOperationSlot();
  const renameOperation = newOperationSlot();
  const endOperation = newOperationSlot();

  const beginOperation = (slot: OperationSlot) => {
    slot.generation += 1;
    slot.controller?.abort();
    slot.controller = new AbortController();
    return { generation: slot.generation, signal: slot.controller.signal };
  };
  const isCurrent = (slot: OperationSlot, generation: number) =>
    slot.generation === generation && !slot.controller?.signal.aborted;
  const cancelOperation = (slot: OperationSlot) => {
    slot.generation += 1;
    slot.controller?.abort();
    slot.controller = null;
  };
  const cancelMutations = () => {
    cancelOperation(createOperation);
    cancelOperation(renameOperation);
    cancelOperation(endOperation);
    creating.value = false;
    ending.value = false;
  };
  const cancelRename = () => cancelOperation(renameOperation);

  const sessionsForTarget = computed(() =>
    sessions.value.filter(
      (session) => session.targetId === selectedTargetId.value,
    ),
  );
  const selectedSession = computed(
    () =>
      sessions.value.find(
        (session) =>
          session.id === selectedSessionId.value &&
          session.targetId === selectedTargetId.value,
      ) ?? null,
  );
  const activeSessionCount = (targetId: string) =>
    sessions.value.filter(
      (session) =>
        session.targetId === targetId && !terminalPhases.has(session.phase),
    ).length;

  const commitSelection = (sessionId: string) => {
    const session = sessions.value.find((item) => item.id === sessionId);
    if (!session) return;
    selectedSessionId.value = session.id;
    try {
      localStorage.setItem(RECENT_SESSION_KEY, session.id);
    } catch {
      // Selection persistence is optional.
    }
  };

  const selectSession = (sessionId: string) => {
    if (sessionId === selectedSessionId.value) return;
    cancelMutations();
    commitSelection(sessionId);
  };

  const reconcileSelection = () => {
    if (
      selectedSession.value?.targetId === selectedTargetId.value &&
      sessionsForTarget.value.some(
        (session) => session.id === selectedSessionId.value,
      )
    ) {
      return;
    }
    const remembered = readRecentSession();
    const next =
      sessionsForTarget.value.find((session) => session.id === remembered) ??
      sessionsForTarget.value[0] ??
      null;
    selectedSessionId.value = next?.id ?? "";
  };

  const applyList = (
    nextRuntimeId: string,
    nextSessions: TerminalSessionRecord[],
  ) => {
    if (runtimeId.value && runtimeId.value !== nextRuntimeId) {
      cancelMutations();
      onRuntimeChanged?.(runtimeId.value, nextRuntimeId);
      selectedSessionId.value = "";
    }
    runtimeId.value = nextRuntimeId;
    sessions.value = nextSessions;
    reconcileSelection();
  };

  const loadSessions = async () => {
    const generation = ++loadGeneration;
    loadController?.abort();
    loadController = new AbortController();
    loading.value = true;
    error.value = "";
    errorCode.value = null;
    try {
      const result = await TerminalAPI.listSessions(loadController.signal);
      if (generation !== loadGeneration) return false;
      applyList(result.runtimeId, result.sessions);
      return true;
    } catch (reason) {
      if (generation !== loadGeneration || loadController.signal.aborted)
        return false;
      const failure = extractTerminalError(reason);
      error.value = failure.message;
      errorCode.value = failure.errorCode;
      throw reason;
    } finally {
      if (generation === loadGeneration) loading.value = false;
    }
  };

  const createSession = async (
    targetId: string,
    dimensions: { cols: number; rows: number },
  ) => {
    const operation = beginOperation(createOperation);
    creating.value = true;
    error.value = "";
    errorCode.value = null;
    try {
      const session = await TerminalAPI.createSession(
        targetId,
        dimensions,
        operation.signal,
      );
      if (!isCurrent(createOperation, operation.generation)) {
        throw new DOMException("Aborted", "AbortError");
      }
      sessions.value = [
        ...sessions.value.filter((item) => item.id !== session.id),
        session,
      ];
      commitSelection(session.id);
      return session;
    } catch (reason) {
      if (isCurrent(createOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    } finally {
      if (isCurrent(createOperation, operation.generation)) {
        creating.value = false;
      }
    }
  };

  const renameSession = async (sessionId: string, title: string) => {
    const operation = beginOperation(renameOperation);
    error.value = "";
    errorCode.value = null;
    try {
      const updated = await TerminalAPI.updateSessionTitle(
        sessionId,
        title,
        operation.signal,
      );
      if (!isCurrent(renameOperation, operation.generation)) {
        throw new DOMException("Aborted", "AbortError");
      }
      sessions.value = sessions.value.map((session) =>
        session.id === updated.id ? updated : session,
      );
      return updated;
    } catch (reason) {
      if (isCurrent(renameOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    }
  };

  const endSession = async (sessionId: string) => {
    const operation = beginOperation(endOperation);
    ending.value = true;
    error.value = "";
    errorCode.value = null;
    try {
      await TerminalAPI.deleteSession(sessionId, operation.signal);
      if (!isCurrent(endOperation, operation.generation)) return;
      sessions.value = sessions.value.filter(
        (session) => session.id !== sessionId,
      );
      if (selectedSessionId.value === sessionId) selectedSessionId.value = "";
      reconcileSelection();
    } catch (reason) {
      if (isCurrent(endOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    } finally {
      if (isCurrent(endOperation, operation.generation)) ending.value = false;
    }
  };

  const updateSessionPhase = (
    sessionId: string,
    phase: TerminalSessionPhase,
    details: {
      errorCode?: TerminalErrorCode | null;
      errorMessage?: string | null;
      exitCode?: number | null;
    } = {},
  ) => {
    sessions.value = sessions.value.map((session) =>
      session.id === sessionId
        ? {
            ...session,
            phase,
            errorCode: details.errorCode ?? session.errorCode,
            errorMessage: details.errorMessage ?? session.errorMessage,
            exitCode: details.exitCode ?? session.exitCode,
          }
        : session,
    );
  };

  const updateSessionDimensions = (
    sessionId: string,
    cols: number,
    rows: number,
  ) => {
    sessions.value = sessions.value.map((session) =>
      session.id === sessionId ? { ...session, cols, rows } : session,
    );
  };

  const removeSessionsForTarget = (targetId: string) => {
    sessions.value = sessions.value.filter(
      (session) => session.targetId !== targetId,
    );
    reconcileSelection();
  };

  const dispose = () => {
    loadGeneration += 1;
    loadController?.abort();
    loadController = null;
    cancelMutations();
  };

  return {
    activeSessionCount,
    createSession,
    creating,
    cancelMutations,
    cancelRename,
    dispose,
    endSession,
    ending,
    error,
    errorCode,
    loadSessions,
    loading,
    reconcileSelection,
    removeSessionsForTarget,
    renameSession,
    runtimeId,
    selectedSession,
    selectedSessionId,
    selectSession,
    sessions,
    sessionsForTarget,
    updateSessionDimensions,
    updateSessionPhase,
  };
};
