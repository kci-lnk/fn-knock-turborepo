import { nextTick, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type {
  TerminalDestination,
  TerminalSessionRecord,
  TerminalTargetRecord,
} from "@/lib/api/terminal";
import { extractTerminalError, localizeTerminalError } from "./terminal-errors";

export const useTerminalSessionActions = ({
  beginTargetCreate,
  beginTargetEdit,
  beginLocalSettings,
  connect,
  createSession: requestCreateSession,
  detach,
  endSession,
  focusTerminal,
  getTerminalSize,
  isAttachedTo,
  onConnectStart,
  reconnectAttachment,
  selectedSession,
  selectedSessionId,
  selectedTarget,
  sessions,
  translate,
}: {
  beginTargetCreate: () => void;
  beginTargetEdit: (target: TerminalTargetRecord) => void;
  beginLocalSettings: () => void;
  connect: (session: TerminalSessionRecord) => Promise<void>;
  createSession: (
    targetId: string,
    size: { cols: number; rows: number },
  ) => Promise<TerminalSessionRecord>;
  detach: () => Promise<void>;
  endSession: (sessionId: string) => Promise<void>;
  focusTerminal: () => void;
  getTerminalSize: () => { cols: number; rows: number };
  isAttachedTo: (sessionId: string) => boolean;
  onConnectStart: () => void;
  reconnectAttachment: () => Promise<void>;
  selectedSession: Readonly<Ref<TerminalSessionRecord | null>>;
  selectedSessionId: Readonly<Ref<string>>;
  selectedTarget: Readonly<Ref<TerminalDestination | null>>;
  sessions: Readonly<Ref<TerminalSessionRecord[]>>;
  translate: (key: string) => string;
}) => {
  const errorMessage = (reason: unknown, fallback: string) =>
    localizeTerminalError(extractTerminalError(reason, fallback), translate);

  const connectToSession = async (session: TerminalSessionRecord) => {
    onConnectStart();
    await connect(session);
  };

  // Output no longer takes focus. Restore it only for explicit session actions,
  // and only if the user has not moved to another field while connecting.
  const restoreUserFocus = (origin: Element | null, sessionId: string) => {
    if (selectedSessionId.value !== sessionId) return;
    if (
      document.activeElement === origin ||
      (origin &&
        !origin.isConnected &&
        document.activeElement === document.body)
    )
      focusTerminal();
  };

  const handleSessionTabChange = async (sessionId: string | number) => {
    const origin = document.activeElement;
    const session = sessions.value.find(
      (item) => item.id === String(sessionId),
    );
    if (!session) return;
    if (session.id === selectedSessionId.value && isAttachedTo(session.id)) {
      focusTerminal();
      return;
    }
    try {
      await connectToSession(session);
      restoreUserFocus(origin, session.id);
    } catch (reason) {
      toast.error(translate("admin.webTerminal.switchFailed"), {
        description: errorMessage(
          reason,
          translate("admin.webTerminal.switchFailedDescription"),
        ),
      });
    }
  };

  const createSession = async () => {
    const origin = document.activeElement;
    const target = selectedTarget.value;
    if (!target) {
      beginTargetCreate();
      return null;
    }
    if (target.kind === "local" && (!target.enabled || !target.ready)) {
      beginLocalSettings();
      return null;
    }
    if (
      target.kind === "ssh" &&
      (!target.credentialConfigured ||
        !target.trustedHostKey ||
        !target.lastVerifiedAt)
    ) {
      beginTargetEdit(target);
      return null;
    }
    try {
      const session = await requestCreateSession(target.id, getTerminalSize());
      await nextTick();
      await connectToSession(session);
      restoreUserFocus(origin, session.id);
      toast.success(translate("admin.webTerminal.sessionCreated"));
      return session;
    } catch (reason) {
      toast.error(translate("admin.webTerminal.createFailed"), {
        description: errorMessage(
          reason,
          translate("admin.webTerminal.createFailedDescription"),
        ),
      });
      return null;
    }
  };

  const destroySelectedSession = async () => {
    const session = selectedSession.value;
    if (!session) return;
    try {
      await detach();
      await endSession(session.id);
      const nextSession = selectedSession.value;
      if (nextSession) await connectToSession(nextSession);
      toast.success(translate("admin.webTerminal.sessionEnded"));
    } catch (reason) {
      toast.error(translate("admin.webTerminal.endFailed"), {
        description: errorMessage(
          reason,
          translate("admin.webTerminal.endFailedDescription"),
        ),
      });
    }
  };

  const reconnectSession = async () => {
    const origin = document.activeElement;
    const sessionId = selectedSessionId.value;
    try {
      await reconnectAttachment();
      restoreUserFocus(origin, sessionId);
    } catch (reason) {
      toast.error(translate("admin.webTerminal.reconnectFailed"), {
        description: errorMessage(
          reason,
          translate("admin.webTerminal.reconnectFailedDescription"),
        ),
      });
    }
  };

  return {
    connectToSession,
    createSession,
    destroySelectedSession,
    handleSessionTabChange,
    reconnectSession,
  };
};
