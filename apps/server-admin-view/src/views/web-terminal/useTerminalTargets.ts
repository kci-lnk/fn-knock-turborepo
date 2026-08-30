import { computed, ref } from "vue";
import {
  TerminalAPI,
  type TerminalDestination,
  type TerminalErrorCode,
  type TerminalLocalStatus,
  type TerminalSshDestination,
  type TerminalTargetCreateInput,
  type TerminalTargetUpdateInput,
} from "@/lib/api/terminal";
import { extractTerminalError } from "./terminal-errors";

const RECENT_TARGET_KEY = "fn-knock:terminal:last-target";

const readRecentTarget = () => {
  try {
    return localStorage.getItem(RECENT_TARGET_KEY) ?? "";
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

export const useTerminalTargets = () => {
  const sshTargets = ref<TerminalSshDestination[]>([]);
  const localStatus = ref<TerminalLocalStatus | null>(null);
  const selectedTargetId = ref("");
  const loading = ref(false);
  const updatingLocal = ref(false);
  const error = ref("");
  const errorCode = ref<TerminalErrorCode | null>(null);
  let loadGeneration = 0;
  let loadController: AbortController | null = null;
  const createOperation = newOperationSlot();
  const updateOperation = newOperationSlot();
  const deleteOperation = newOperationSlot();
  const localOperation = newOperationSlot();

  const targets = computed<TerminalDestination[]>(() => {
    const local = localStatus.value;
    return [
      ...(local?.supported
        ? [
            {
              ...local,
              id: "local" as const,
              kind: "local" as const,
              name: "Local" as const,
            },
          ]
        : []),
      ...sshTargets.value,
    ];
  });

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
    cancelOperation(updateOperation);
    cancelOperation(deleteOperation);
    cancelOperation(localOperation);
  };
  const cancelEdits = () => {
    cancelOperation(createOperation);
    cancelOperation(updateOperation);
  };

  const selectedTarget = computed(
    () =>
      targets.value.find((target) => target.id === selectedTargetId.value) ??
      null,
  );

  const commitSelection = (targetId: string) => {
    if (!targets.value.some((target) => target.id === targetId)) return;
    selectedTargetId.value = targetId;
    try {
      localStorage.setItem(RECENT_TARGET_KEY, targetId);
    } catch {
      // Selection persistence is optional.
    }
  };

  const selectTarget = (targetId: string) => {
    if (targetId === selectedTargetId.value) return;
    cancelMutations();
    commitSelection(targetId);
  };

  const reconcileSelection = () => {
    if (
      selectedTargetId.value &&
      targets.value.some((target) => target.id === selectedTargetId.value)
    ) {
      return;
    }
    const recentTarget = readRecentTarget();
    const next =
      targets.value.find((target) => target.id === recentTarget) ??
      targets.value[0] ??
      null;
    selectedTargetId.value = next?.id ?? "";
  };

  const loadTargets = async () => {
    const generation = ++loadGeneration;
    loadController?.abort();
    loadController = new AbortController();
    loading.value = true;
    error.value = "";
    errorCode.value = null;
    try {
      const [localResult, sshResult] = await Promise.allSettled([
        TerminalAPI.getLocalStatus(loadController.signal),
        TerminalAPI.listTargets(loadController.signal),
      ]);
      if (generation !== loadGeneration || loadController.signal.aborted)
        return;
      if (sshResult.status === "rejected") throw sshResult.reason;
      sshTargets.value = sshResult.value.map((target) => ({
        ...target,
        kind: "ssh",
      }));
      if (localResult.status === "fulfilled") {
        localStatus.value = localResult.value;
      } else {
        // The local backend is optional. Fail it closed without hiding SSH
        // destinations that were loaded successfully.
        localStatus.value = null;
        const failure = extractTerminalError(localResult.reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      reconcileSelection();
    } catch (reason) {
      if (generation !== loadGeneration || loadController.signal.aborted)
        return;
      const failure = extractTerminalError(reason);
      error.value = failure.message;
      errorCode.value = failure.errorCode;
      throw reason;
    } finally {
      if (generation === loadGeneration) loading.value = false;
    }
  };

  const createTarget = async (payload: TerminalTargetCreateInput) => {
    const operation = beginOperation(createOperation);
    error.value = "";
    errorCode.value = null;
    try {
      const created = await TerminalAPI.createTarget(payload, operation.signal);
      if (!isCurrent(createOperation, operation.generation)) {
        throw new DOMException("Aborted", "AbortError");
      }
      sshTargets.value = [...sshTargets.value, { ...created, kind: "ssh" }];
      commitSelection(created.id);
      return created;
    } catch (reason) {
      if (isCurrent(createOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    }
  };

  const updateTarget = async (
    targetId: string,
    payload: TerminalTargetUpdateInput,
    force = false,
    confirmationToken?: string,
  ) => {
    const operation = beginOperation(updateOperation);
    error.value = "";
    errorCode.value = null;
    try {
      const updated = await TerminalAPI.updateTarget(
        targetId,
        payload,
        force,
        confirmationToken,
        operation.signal,
      );
      if (!isCurrent(updateOperation, operation.generation)) {
        throw new DOMException("Aborted", "AbortError");
      }
      sshTargets.value = sshTargets.value.map((target) =>
        target.id === updated.id ? { ...updated, kind: "ssh" } : target,
      );
      return updated;
    } catch (reason) {
      if (isCurrent(updateOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    }
  };

  const deleteTarget = async (
    targetId: string,
    revision: number,
    force = false,
    confirmationToken?: string,
  ) => {
    const operation = beginOperation(deleteOperation);
    error.value = "";
    errorCode.value = null;
    try {
      await TerminalAPI.deleteTarget(
        targetId,
        revision,
        force,
        confirmationToken,
        operation.signal,
      );
      if (!isCurrent(deleteOperation, operation.generation)) return;
      sshTargets.value = sshTargets.value.filter(
        (target) => target.id !== targetId,
      );
      reconcileSelection();
    } catch (reason) {
      if (isCurrent(deleteOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    }
  };

  const updateLocalTerminal = async (
    enabled: boolean,
    acknowledgeRisk = false,
    force = false,
    confirmationToken?: string,
  ) => {
    const status = localStatus.value;
    if (!status) throw new Error("local terminal status is unavailable");
    const operation = beginOperation(localOperation);
    updatingLocal.value = true;
    error.value = "";
    errorCode.value = null;
    try {
      const updated = await TerminalAPI.updateLocalStatus(
        {
          enabled,
          revision: status.revision,
          acknowledgeRisk,
        },
        force,
        confirmationToken,
        operation.signal,
      );
      if (!isCurrent(localOperation, operation.generation)) {
        throw new DOMException("Aborted", "AbortError");
      }
      localStatus.value = updated;
      reconcileSelection();
      return updated;
    } catch (reason) {
      if (isCurrent(localOperation, operation.generation)) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      throw reason;
    } finally {
      if (isCurrent(localOperation, operation.generation)) {
        updatingLocal.value = false;
      }
    }
  };

  const dispose = () => {
    loadGeneration += 1;
    loadController?.abort();
    loadController = null;
    cancelMutations();
  };

  return {
    createTarget,
    cancelEdits,
    cancelMutations,
    deleteTarget,
    dispose,
    error,
    errorCode,
    loadTargets,
    localStatus,
    loading,
    selectedTarget,
    selectedTargetId,
    selectTarget,
    targets,
    updateLocalTerminal,
    updatingLocal,
    updateTarget,
  };
};
