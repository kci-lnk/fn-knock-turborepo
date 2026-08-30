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

  const handleSessionTabChange = async (sessionId: string | number) => {
    const session = sessions.value.find(
      (item) => item.id === String(sessionId),
    );
    if (!session) return;
    if (session.id === selectedSessionId.value && isAttachedTo(session.id)) {
      return;
    }
    try {
      await connectToSession(session);
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
    try {
      await reconnectAttachment();
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
