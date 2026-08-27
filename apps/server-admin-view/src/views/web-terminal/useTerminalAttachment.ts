import { computed, ref } from "vue";
import {
  TerminalAPI,
  type TerminalAttachmentRecord,
  type TerminalErrorCode,
  type TerminalOutputEvent,
  type TerminalSessionPhase,
  type TerminalSessionRecord,
} from "@/lib/api/terminal";
import { extractTerminalError } from "./terminal-errors";
import { normalizeTerminalDimensions } from "./terminal-dimensions";

export type TerminalAttachmentState =
  | { kind: "idle" }
  | { kind: "opening"; sessionId: string }
  | { kind: "snapshotting"; sessionId: string }
  | { kind: "controller"; sessionId: string }
  | { kind: "viewer"; sessionId: string }
  | { kind: "reconnecting"; sessionId: string; attempt: number }
  | { kind: "error"; sessionId: string; message: string };

const isTerminalPhase = (phase: TerminalSessionPhase) =>
  phase === "closed" ||
  phase === "exited" ||
  phase === "lost" ||
  phase === "failed";

const phaseOrder: Record<TerminalSessionPhase, number> = {
  creating: 0,
  resolving: 1,
  connecting: 2,
  verifyingHostKey: 3,
  authenticating: 4,
  openingChannel: 5,
  requestingPty: 6,
  running: 7,
  closing: 8,
  closed: 9,
  exited: 9,
  lost: 9,
  failed: 9,
};

const canAdvancePhase = (
  current: TerminalSessionPhase | null,
  next: TerminalSessionPhase,
) =>
  current === null ||
  current === next ||
  (!isTerminalPhase(current) && phaseOrder[next] >= phaseOrder[current]);

const wait = (delay: number, signal: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    if (signal.aborted) {
      reject(new DOMException("Aborted", "AbortError"));
      return;
    }
    const onAbort = () => {
      window.clearTimeout(timer);
      reject(new DOMException("Aborted", "AbortError"));
    };
    const timer = window.setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, delay);
    signal.addEventListener("abort", onAbort, { once: true });
  });

export const useTerminalAttachment = ({
  getTerminalSize,
  onOutput,
  onReset,
  onSessionState,
}: {
  getTerminalSize: () => { cols: number; rows: number };
  onOutput: (event: TerminalOutputEvent) => void;
  onReset: () => void;
  onSessionState: (
    sessionId: string,
    phase: TerminalSessionPhase,
    details: {
      errorCode?: TerminalErrorCode | null;
      errorMessage?: string | null;
      exitCode?: number | null;
    },
  ) => void;
}) => {
  const attachment = ref<TerminalAttachmentRecord | null>(null);
  const state = ref<TerminalAttachmentState>({ kind: "idle" });
  const cursor = ref(0);
  const lastError = ref("");
  const lastErrorCode = ref<TerminalErrorCode | null>(null);
  const livePhase = ref<TerminalSessionPhase | null>(null);
  const terminalPhase = computed(
    () => livePhase.value !== null && isTerminalPhase(livePhase.value),
  );
  const connected = computed(
    () => state.value.kind === "controller" || state.value.kind === "viewer",
  );
  const inputConnectionState = computed<
    "idle" | "connecting" | "connected" | "error"
  >(() => {
    const kind = state.value.kind;
    if (kind === "controller" || kind === "viewer") return "connected";
    if (
      kind === "opening" ||
      kind === "snapshotting" ||
      kind === "reconnecting"
    ) {
      return "connecting";
    }
    return kind === "error" ? "error" : "idle";
  });
  const canInput = computed(
    () =>
      state.value.kind === "controller" &&
      livePhase.value === "running" &&
      attachment.value?.role === "controller",
  );
  const readOnly = computed(
    () =>
      livePhase.value !== "running" ||
      state.value.kind === "viewer" ||
      attachment.value?.role === "viewer",
  );
  const canClaimControl = computed(
    () =>
      livePhase.value === "running" &&
      state.value.kind === "viewer" &&
      attachment.value?.role === "viewer",
  );
  const sessionId = computed(() =>
    state.value.kind === "idle" ? "" : state.value.sessionId,
  );
  const isAttachedTo = (candidate: string) =>
    sessionId.value === candidate &&
    state.value.kind !== "idle" &&
    state.value.kind !== "error";
  let lifecycleGeneration = 0;
  let lifecycleController: AbortController | null = null;
  let desiredSession: TerminalSessionRecord | null = null;
  let inputSequence = 0;
  let resizeRevision = 0;

  const setReadyState = (record: TerminalAttachmentRecord) => {
    state.value = {
      kind:
        !terminalPhase.value && record.role === "controller"
          ? "controller"
          : "viewer",
      sessionId: record.sessionId,
    };
  };

  const applyEvents = (
    record: TerminalAttachmentRecord,
    events: Awaited<ReturnType<typeof TerminalAPI.pollAttachmentEvents>>,
  ) => {
    for (const event of events.events) {
      if (event.type === "output") {
        if (event.reset) onReset();
        onOutput(event);
        continue;
      }
      if (event.type === "control") {
        const current = attachment.value;
        if (
          current?.id === record.id &&
          event.generation >= current.generation
        ) {
          attachment.value = {
            ...current,
            role: event.role,
            generation: event.generation,
          };
          setReadyState(attachment.value);
        }
        continue;
      }
      onSessionState(record.sessionId, event.phase, {
        errorCode: event.errorCode,
        errorMessage: event.errorMessage,
        exitCode: event.exitCode,
      });
      livePhase.value = event.phase;
      if (desiredSession?.id === record.sessionId) {
        desiredSession = { ...desiredSession, phase: event.phase };
      }
      if (isTerminalPhase(event.phase)) {
        setReadyState(record);
        if (event.errorMessage) lastError.value = event.errorMessage;
        lastErrorCode.value = event.errorCode ?? null;
      }
    }
    cursor.value = events.nextCursor;
  };

  const openAttachment = async (
    session: TerminalSessionRecord,
    generation: number,
    signal: AbortSignal,
  ) => {
    const record = await TerminalAPI.createAttachment(
      session.id,
      normalizeTerminalDimensions(getTerminalSize()),
      signal,
    );
    if (generation !== lifecycleGeneration || signal.aborted) {
      await TerminalAPI.detachAttachment(record.id).catch(() => undefined);
      return null;
    }
    attachment.value = record;
    inputSequence = 0;
    resizeRevision = 0;
    cursor.value = record.cursor;
    state.value = { kind: "snapshotting", sessionId: session.id };
    onReset();
    return record;
  };

  const pollUntilStopped = async (
    session: TerminalSessionRecord,
    firstRecord: TerminalAttachmentRecord,
    generation: number,
    signal: AbortSignal,
  ) => {
    let record = firstRecord;
    let reconnectAttempt = 0;

    while (generation === lifecycleGeneration && !signal.aborted) {
      try {
        const result = await TerminalAPI.pollAttachmentEvents(
          record.id,
          { after: cursor.value, timeoutMs: 4500 },
          signal,
        );
        if (generation !== lifecycleGeneration || signal.aborted) return;
        reconnectAttempt = 0;
        lastError.value = "";
        lastErrorCode.value = null;
        applyEvents(record, result);
        if (state.value.kind === "snapshotting") setReadyState(record);
      } catch (reason) {
        if (generation !== lifecycleGeneration || signal.aborted) return;
        reconnectAttempt += 1;
        const failure = extractTerminalError(reason);
        lastError.value = failure.message;
        lastErrorCode.value = failure.errorCode;
        state.value = {
          kind: "reconnecting",
          sessionId: session.id,
          attempt: reconnectAttempt,
        };
        attachment.value = null;
        await TerminalAPI.detachAttachment(record.id).catch(() => undefined);
        try {
          await wait(Math.min(4000, 500 * 2 ** (reconnectAttempt - 1)), signal);
          const replacement = await openAttachment(session, generation, signal);
          if (!replacement) return;
          record = replacement;
        } catch (reconnectError) {
          if (generation !== lifecycleGeneration || signal.aborted) return;
          const failure = extractTerminalError(reconnectError);
          lastError.value = failure.message;
          lastErrorCode.value = failure.errorCode;
        }
      }
    }
  };

  const attach = async (session: TerminalSessionRecord) => {
    const previousAttachment = attachment.value;
    lifecycleGeneration += 1;
    lifecycleController?.abort();
    lifecycleController = new AbortController();
    desiredSession = session;
    livePhase.value = session.phase;
    attachment.value = null;
    cursor.value = 0;
    lastError.value = "";
    lastErrorCode.value = null;
    state.value = { kind: "opening", sessionId: session.id };
    if (previousAttachment) {
      void TerminalAPI.detachAttachment(previousAttachment.id).catch(
        () => undefined,
      );
    }

    const generation = lifecycleGeneration;
    try {
      const record = await openAttachment(
        session,
        generation,
        lifecycleController.signal,
      );
      if (!record) return;
      void pollUntilStopped(
        session,
        record,
        generation,
        lifecycleController.signal,
      );
    } catch (reason) {
      if (
        generation !== lifecycleGeneration ||
        lifecycleController.signal.aborted
      ) {
        return;
      }
      const failure = extractTerminalError(reason);
      lastError.value = failure.message;
      lastErrorCode.value = failure.errorCode;
      state.value = {
        kind: "error",
        sessionId: session.id,
        message: lastError.value,
      };
      throw reason;
    }
  };

  const reconnect = async () => {
    if (!desiredSession) return;
    await attach(desiredSession);
  };

  const reportRequestError = (reason: unknown, fallback?: string) => {
    const failure = extractTerminalError(reason, fallback);
    lastError.value = failure.message;
    lastErrorCode.value = failure.errorCode;
    const current = attachment.value;
    if (current && failure.errorCode === "controller_conflict") {
      attachment.value = { ...current, role: "viewer" };
      setReadyState(attachment.value);
    }
    return failure;
  };

  const syncSession = (session: TerminalSessionRecord) => {
    if (desiredSession?.id !== session.id) return;
    if (!canAdvancePhase(livePhase.value, session.phase)) {
      desiredSession = {
        ...session,
        phase: livePhase.value ?? session.phase,
      };
      return;
    }
    desiredSession = session;
    livePhase.value = session.phase;
    const current = attachment.value;
    if (current && isTerminalPhase(session.phase)) setReadyState(current);
  };

  const claimControl = async () => {
    const current = attachment.value;
    if (!current || !canClaimControl.value) return;
    const signal = lifecycleController?.signal;
    if (!signal || signal.aborted) return;
    try {
      const claimed = await TerminalAPI.claimControl(
        current.id,
        current.generation,
        signal,
      );
      if (attachment.value?.id !== current.id) return;
      attachment.value = claimed;
      lastError.value = "";
      lastErrorCode.value = null;
      setReadyState(claimed);
    } catch (reason) {
      if (attachment.value?.id !== current.id) return;
      const failure = extractTerminalError(reason);
      lastError.value = failure.message;
      lastErrorCode.value = failure.errorCode;
    }
  };

  const sendInput = async (dataBase64: string) => {
    const current = attachment.value;
    const signal = lifecycleController?.signal;
    if (!current || !canInput.value || !signal || signal.aborted) {
      throw new Error("This terminal attachment is read-only");
    }
    inputSequence += 1;
    const payload = {
      dataBase64,
      sequence: inputSequence,
      generation: current.generation,
    };
    try {
      await TerminalAPI.sendInput(current.id, payload, signal);
    } catch (reason) {
      const latest = attachment.value;
      if (
        !canInput.value ||
        latest?.id !== current.id ||
        latest.generation !== current.generation
      ) {
        throw reason;
      }
      await TerminalAPI.sendInput(current.id, payload, signal);
    }
  };

  const resize = async (cols: number, rows: number) => {
    const current = attachment.value;
    const signal = lifecycleController?.signal;
    if (!current || !canInput.value || !signal || signal.aborted) return;
    resizeRevision += 1;
    const dimensions = normalizeTerminalDimensions({ cols, rows });
    try {
      await TerminalAPI.resizeAttachment(
        current.id,
        {
          ...dimensions,
          revision: resizeRevision,
          generation: current.generation,
        },
        signal,
      );
    } catch (reason) {
      if (attachment.value?.id === current.id) {
        const failure = extractTerminalError(reason);
        lastError.value = failure.message;
        lastErrorCode.value = failure.errorCode;
      }
      throw reason;
    }
  };

  const detach = async () => {
    const current = attachment.value;
    lifecycleGeneration += 1;
    lifecycleController?.abort();
    lifecycleController = null;
    attachment.value = null;
    desiredSession = null;
    livePhase.value = null;
    cursor.value = 0;
    state.value = { kind: "idle" };
    if (current) {
      await TerminalAPI.detachAttachment(current.id).catch(() => undefined);
    }
  };

  const dispose = () => detach();

  return {
    attach,
    attachment,
    canClaimControl,
    canInput,
    claimControl,
    connected,
    cursor,
    detach,
    dispose,
    lastError,
    lastErrorCode,
    livePhase,
    isAttachedTo,
    inputConnectionState,
    readOnly,
    reconnect,
    reportRequestError,
    resize,
    sendInput,
    sessionId,
    state,
    syncSession,
  };
};
