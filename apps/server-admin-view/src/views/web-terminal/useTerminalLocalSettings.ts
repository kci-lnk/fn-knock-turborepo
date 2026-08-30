import { computed, ref, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type {
  TerminalLocalStatus,
  TerminalSessionRecord,
} from "@/lib/api/terminal";
import { extractTerminalError, localizeTerminalError } from "./terminal-errors";

export const useTerminalLocalSettings = ({
  activeSessionCount,
  attachedSessionId,
  detach,
  loadSessions,
  localStatus,
  localUpdating,
  sessions,
  translate,
  updateLocalTerminal,
}: {
  activeSessionCount: (targetId: string) => number;
  attachedSessionId: Readonly<Ref<string>>;
  detach: () => Promise<void>;
  loadSessions: () => Promise<boolean>;
  localStatus: Ref<TerminalLocalStatus | null>;
  localUpdating: Readonly<Ref<boolean>>;
  sessions: Readonly<Ref<TerminalSessionRecord[]>>;
  translate: (key: string) => string;
  updateLocalTerminal: (
    enabled: boolean,
    acknowledgeRisk?: boolean,
    force?: boolean,
    confirmationToken?: string,
  ) => Promise<TerminalLocalStatus>;
}) => {
  const localSettingsOpen = ref(false);
  const localRiskAcknowledged = ref(false);
  const localSettingsError = ref("");
  const localConfirmationToken = ref("");
  const localConflictingSessionCount = ref(0);

  const resetDialog = () => {
    localRiskAcknowledged.value = false;
    localSettingsError.value = "";
    localConfirmationToken.value = "";
    localConflictingSessionCount.value = 0;
  };
  const openLocalSettings = () => {
    resetDialog();
    localSettingsOpen.value = true;
  };
  const closeLocalSettings = () => {
    if (localUpdating.value) return;
    localSettingsOpen.value = false;
    resetDialog();
  };
  const submitLocalSettings = async (force = false) => {
    const status = localStatus.value;
    if (!status) return;
    const enabled = !status.enabled;
    localSettingsError.value = "";
    try {
      await updateLocalTerminal(
        enabled,
        enabled && localRiskAcknowledged.value,
        force,
        force ? localConfirmationToken.value : undefined,
      );
      if (!enabled) {
        const attachedSession = sessions.value.find(
          (session) => session.id === attachedSessionId.value,
        );
        if (attachedSession?.targetId === "local") await detach();
        await loadSessions();
      }
      localSettingsOpen.value = false;
      resetDialog();
      toast.success(
        translate(
          enabled
            ? "admin.webTerminal.localEnabled"
            : "admin.webTerminal.localDisabled",
        ),
      );
    } catch (reason) {
      const failure = extractTerminalError(reason);
      if (
        !enabled &&
        failure.errorCode === "conflict" &&
        failure.confirmationToken
      ) {
        localConfirmationToken.value = failure.confirmationToken;
        localConflictingSessionCount.value =
          failure.activeSessionCount ?? activeSessionCount("local");
      } else {
        localConfirmationToken.value = "";
        localConflictingSessionCount.value = 0;
      }
      localSettingsError.value = localizeTerminalError(failure, translate);
    }
  };

  return {
    closeLocalSettings,
    localActiveSessionCount: computed(() => activeSessionCount("local")),
    localConfirmationRequired: computed(() => !!localConfirmationToken.value),
    localConflictingSessionCount,
    localRiskAcknowledged,
    localSettingsError,
    localSettingsOpen,
    localStatus,
    localUpdating,
    openLocalSettings,
    submitLocalSettings,
  };
};
