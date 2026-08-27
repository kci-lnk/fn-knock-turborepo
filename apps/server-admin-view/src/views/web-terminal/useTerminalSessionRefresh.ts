import type { Ref } from "vue";
import type { TerminalSessionRecord } from "@/lib/api/terminal";

export const useTerminalSessionRefresh =
  ({
    attachmentSessionId,
    connectToSession,
    detach,
    isDisposed,
    loadSessions,
    runtimeRestarted,
    selectedSession,
    sessionExists,
  }: {
    attachmentSessionId: Ref<string>;
    connectToSession: (session: TerminalSessionRecord) => Promise<void>;
    detach: () => Promise<void>;
    isDisposed: () => boolean;
    loadSessions: () => Promise<boolean>;
    runtimeRestarted: Ref<boolean>;
    selectedSession: Ref<TerminalSessionRecord | null>;
    sessionExists: (sessionId: string) => boolean;
  }) =>
  async () => {
    const previousSessionId = attachmentSessionId.value;
    const applied = await loadSessions();
    if (
      isDisposed() ||
      !applied ||
      !previousSessionId ||
      attachmentSessionId.value !== previousSessionId ||
      sessionExists(previousSessionId)
    ) {
      return;
    }
    await detach();
    if (!runtimeRestarted.value && selectedSession.value) {
      await connectToSession(selectedSession.value);
    }
  };
