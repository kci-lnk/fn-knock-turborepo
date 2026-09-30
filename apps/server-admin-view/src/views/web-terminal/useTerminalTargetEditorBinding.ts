import { useTerminalTargetEditor } from "./useTerminalTargetEditor";
import type { useTerminalTargets } from "./useTerminalTargets";
import type { useTerminalSessions } from "./useTerminalSessions";
import type { useTerminalAttachment } from "./useTerminalAttachment";
import type { useTerminalSessionConnection } from "./useTerminalSessionConnection";

export function useTerminalTargetEditorBinding(
  targetsController: ReturnType<typeof useTerminalTargets>,
  sessionsController: ReturnType<typeof useTerminalSessions>,
  attachmentController: ReturnType<typeof useTerminalAttachment>,
  sessionConnection: ReturnType<typeof useTerminalSessionConnection>,
) {
  return useTerminalTargetEditor({
    cancelPendingSave: targetsController.cancelEdits,
    createTarget: targetsController.createTarget,
    updateTarget: async (targetId, payload, force, confirmationToken) => {
      const updated = await targetsController.updateTarget(
        targetId,
        payload,
        force,
        confirmationToken,
      );
      sessionsController.applyTargetPersistence(targetId, updated.persistent);
      if (force) {
        const attachedSession = sessionsController.sessions.value.find(
          (session) => session.id === attachmentController.sessionId.value,
        );
        if (attachedSession?.targetId === targetId) {
          await sessionConnection.detach();
        }
        await sessionsController.loadSessions();
      }
      return updated;
    },
  });
}
