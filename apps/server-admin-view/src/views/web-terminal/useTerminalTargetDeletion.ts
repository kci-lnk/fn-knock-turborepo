import { computed, ref, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type {
  TerminalSessionRecord,
  TerminalTargetRecord,
} from "@/lib/api/terminal";
import { extractTerminalError } from "./terminal-errors";

export const useTerminalTargetDeletion = ({
  activeSessionCount,
  attachedSessionId,
  deleteTarget: requestDeleteTarget,
  detach,
  removeSessionsForTarget,
  sessions,
  translate,
}: {
  activeSessionCount: (targetId: string) => number;
  attachedSessionId: Readonly<Ref<string>>;
  deleteTarget: (
    targetId: string,
    revision: number,
    force: boolean,
    confirmationToken?: string,
  ) => Promise<void>;
  detach: () => Promise<void>;
  removeSessionsForTarget: (targetId: string) => void;
  sessions: Readonly<Ref<TerminalSessionRecord[]>>;
  translate: (key: string, fallback: string) => string;
}) => {
  const pendingForceDeleteTarget = ref<TerminalTargetRecord | null>(null);
  const pendingForceDeleteMessage = ref("");
  const pendingForceDeleteActiveCount = ref(0);
  const pendingForceDeleteConfirmationToken = ref("");
  const forceDeletingTarget = ref(false);
  let operationGeneration = 0;

  const resetPrompt = () => {
    pendingForceDeleteTarget.value = null;
    pendingForceDeleteMessage.value = "";
    pendingForceDeleteActiveCount.value = 0;
    pendingForceDeleteConfirmationToken.value = "";
  };

  const finishTargetDeletion = async (
    target: TerminalTargetRecord,
    generation: number,
  ) => {
    const attachedSession = sessions.value.find(
      (session) => session.id === attachedSessionId.value,
    );
    if (attachedSession?.targetId === target.id) await detach();
    if (generation !== operationGeneration) return;
    removeSessionsForTarget(target.id);
    toast.success(
      translate("admin.webTerminal.targetDeleted", "SSH target deleted"),
    );
  };

  const deleteTarget = async (target: TerminalTargetRecord) => {
    const generation = ++operationGeneration;
    try {
      await requestDeleteTarget(target.id, target.revision, false, undefined);
      if (generation !== operationGeneration) return;
      await finishTargetDeletion(target, generation);
    } catch (reason) {
      if (generation !== operationGeneration) return;
      const failure = extractTerminalError(
        reason,
        translate("admin.webTerminal.targetDeleteFailed", "Delete failed"),
      );
      if (failure.errorCode === "conflict" && failure.confirmationToken) {
        pendingForceDeleteTarget.value = target;
        pendingForceDeleteMessage.value = failure.message;
        pendingForceDeleteConfirmationToken.value = failure.confirmationToken;
        pendingForceDeleteActiveCount.value =
          failure.activeSessionCount ?? activeSessionCount(target.id);
        return;
      }
      toast.error(
        translate("admin.webTerminal.targetDeleteFailed", "Delete failed"),
        { description: failure.message },
      );
    }
  };

  const closeForceDeleteTarget = () => {
    if (forceDeletingTarget.value) return;
    operationGeneration += 1;
    resetPrompt();
  };

  const confirmForceDeleteTarget = async () => {
    const target = pendingForceDeleteTarget.value;
    const confirmationToken = pendingForceDeleteConfirmationToken.value;
    if (!target || !confirmationToken || forceDeletingTarget.value) return;
    const generation = ++operationGeneration;
    forceDeletingTarget.value = true;
    try {
      await requestDeleteTarget(
        target.id,
        target.revision,
        true,
        confirmationToken,
      );
      if (generation !== operationGeneration) return;
      await finishTargetDeletion(target, generation);
      if (generation === operationGeneration) resetPrompt();
    } catch (reason) {
      if (generation !== operationGeneration) return;
      const failure = extractTerminalError(
        reason,
        translate("admin.webTerminal.targetDeleteFailed", "Delete failed"),
      );
      pendingForceDeleteMessage.value = failure.message;
      if (failure.confirmationToken) {
        pendingForceDeleteConfirmationToken.value = failure.confirmationToken;
      } else {
        pendingForceDeleteConfirmationToken.value = "";
      }
      if (failure.activeSessionCount !== null) {
        pendingForceDeleteActiveCount.value = failure.activeSessionCount;
      }
    } finally {
      if (generation === operationGeneration) forceDeletingTarget.value = false;
    }
  };

  const dispose = () => {
    operationGeneration += 1;
    resetPrompt();
  };

  return {
    canConfirmForceDelete: computed(() =>
      Boolean(pendingForceDeleteConfirmationToken.value),
    ),
    closeForceDeleteTarget,
    confirmForceDeleteTarget,
    deleteTarget,
    dispose,
    forceDeletingTarget,
    pendingForceDeleteActiveCount,
    pendingForceDeleteConfirmationToken,
    pendingForceDeleteMessage,
    pendingForceDeleteTarget,
  };
};
