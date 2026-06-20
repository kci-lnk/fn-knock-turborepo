import { nextTick, ref, type ComputedRef, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type {
  TerminalAttachmentRecord,
  TerminalSessionRecord,
} from "@/types";
import { focusElementWithoutScroll } from "./terminal-dom";

export const useTerminalDialogs = ({
  activeAttachment,
  clearArmedModifier,
  focusTerminal,
  selectedSession,
  sendPayloadNow,
  sessions,
  translate,
  updateSessionTitle,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  clearArmedModifier: () => void;
  focusTerminal: () => void;
  selectedSession: ComputedRef<TerminalSessionRecord | null>;
  sendPayloadNow: (payload: string) => Promise<void>;
  sessions: Ref<TerminalSessionRecord[]>;
  translate: (key: string) => string;
  updateSessionTitle: (
    sessionId: string,
    title: string,
  ) => Promise<TerminalSessionRecord>;
}) => {
  const sendDialogOpen = ref(false);
  const sendDialogPayload = ref("");
  const isSendingDialogPayload = ref(false);
  const renameDialogOpen = ref(false);
  const renameDialogValue = ref("");
  const isRenamingSession = ref(false);

  const focusSendDialogTextarea = () => {
    void nextTick(() => {
      const textarea = document.getElementById("terminal-send-payload");
      if (textarea instanceof HTMLElement) {
        focusElementWithoutScroll(textarea);
      }
    });
  };

  const focusTerminalAfterDialogClose = (event: Event) => {
    event.preventDefault();
    void nextTick(() => {
      focusTerminal();
      window.requestAnimationFrame(() => focusTerminal());
    });
  };

  const openSendDialog = () => {
    if (!activeAttachment.value) return;
    sendDialogOpen.value = true;
  };

  const openManualPasteDialog = () => {
    if (!activeAttachment.value) return;
    sendDialogPayload.value = "";
    sendDialogOpen.value = true;
    focusSendDialogTextarea();
    toast.info(translate("admin.webTerminal.manualPasteInfo"));
  };

  const openRenameDialog = () => {
    if (!selectedSession.value) return;
    renameDialogValue.value = selectedSession.value.title;
    renameDialogOpen.value = true;
  };

  const submitRenameDialog = async () => {
    const targetSession = selectedSession.value;
    const nextTitle = renameDialogValue.value.trim();
    if (!targetSession || !nextTitle) return;

    isRenamingSession.value = true;
    try {
      const updatedSession = await updateSessionTitle(
        targetSession.id,
        nextTitle,
      );
      sessions.value = sessions.value.map((session) =>
        session.id === updatedSession.id ? updatedSession : session,
      );
      renameDialogOpen.value = false;
      focusTerminal();
    } catch (error) {
      toast.error(translate("admin.webTerminal.renameFailed"), {
        description:
          error instanceof Error
            ? error.message
            : translate("admin.webTerminal.renameFailedDescription"),
      });
    } finally {
      isRenamingSession.value = false;
    }
  };

  const submitSendDialog = async () => {
    const payload = sendDialogPayload.value;
    if (!payload.length) return;

    isSendingDialogPayload.value = true;
    try {
      clearArmedModifier();
      await sendPayloadNow(payload);
      sendDialogPayload.value = "";
      sendDialogOpen.value = false;
      focusTerminal();
    } catch (error) {
      toast.error(translate("admin.webTerminal.sendFailed"), {
        description:
          error instanceof Error
            ? error.message
            : translate("admin.webTerminal.sendFailedDescription"),
      });
    } finally {
      isSendingDialogPayload.value = false;
    }
  };

  return {
    focusTerminalAfterDialogClose,
    isRenamingSession,
    isSendingDialogPayload,
    openManualPasteDialog,
    openRenameDialog,
    openSendDialog,
    renameDialogOpen,
    renameDialogValue,
    sendDialogOpen,
    sendDialogPayload,
    submitRenameDialog,
    submitSendDialog,
  };
};
