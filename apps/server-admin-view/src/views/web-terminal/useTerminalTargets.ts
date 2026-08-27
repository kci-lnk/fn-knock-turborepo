import { computed, ref } from "vue";
import {
  TerminalAPI,
  type TerminalErrorCode,
  type TerminalTargetCreateInput,
  type TerminalTargetRecord,
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
  const targets = ref<TerminalTargetRecord[]>([]);
  const selectedTargetId = ref("");
  const loading = ref(false);
  const error = ref("");
  const errorCode = ref<TerminalErrorCode | null>(null);
  let loadGeneration = 0;
  let loadController: AbortController | null = null;
  const createOperation = newOperationSlot();
  const updateOperation = newOperationSlot();
  const deleteOperation = newOperationSlot();

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
      const result = await TerminalAPI.listTargets(loadController.signal);
      if (generation !== loadGeneration) return;
      targets.value = result;
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
      targets.value = [...targets.value, created];
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
      targets.value = targets.value.map((target) =>
        target.id === updated.id ? updated : target,
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
      targets.value = targets.value.filter((target) => target.id !== targetId);
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
    loading,
    selectedTarget,
    selectedTargetId,
    selectTarget,
    targets,
    updateTarget,
  };
};
