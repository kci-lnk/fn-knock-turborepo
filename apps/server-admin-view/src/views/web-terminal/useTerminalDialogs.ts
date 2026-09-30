import {
  computed,
  nextTick,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from "vue";
import { toast } from "@admin-shared/utils/toast";
import type {
  TerminalAttachmentRecord,
  TerminalSessionRecord,
} from "@/lib/api/terminal";
import { focusElementWithoutScroll } from "./terminal-dom";
import { extractTerminalErrorMessage } from "./terminal-errors";

export const useTerminalDialogs = ({
  activeAttachment,
  cancelRenameSession,
  clearArmedModifier,
  focusTerminal,
  selectedSession,
  sendPayloadNow,
  sessions,
  translate,
  updateSessionTitle,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  cancelRenameSession: () => void;
  clearArmedModifier: () => void;
  focusTerminal: () => void;
  selectedSession: ComputedRef<TerminalSessionRecord | null>;
  sendPayloadNow: (payload: string) => Promise<void>;
  sessions: Ref<TerminalSessionRecord[]>;
  translate: (key: string) => string;
  updateSessionTitle: (
    sessionId: string,
    title: string,
    persistent?: boolean,
  ) => Promise<TerminalSessionRecord>;
}) => {
  const sendDialogOpen = ref(false);
  const sendDialogPayload = ref("");
  const isSendingDialogPayload = ref(false);
  const renameDialogOpen = ref(false);
  const renameDialogValue = ref("");
  const isRenamingSession = ref(false);
  const renameDialogPersistent = ref(true);
  const editingSessionId = ref("");
  let initialPersistent = true;
  let editGeneration = 0;
  const editingSession = computed(() =>
    sessions.value.find((session) => session.id === editingSessionId.value),
  );
  const renameDialogPersistenceDisabled = computed(
    () =>
      !editingSession.value ||
      ["closing", "closed", "exited", "lost", "failed"].includes(
        editingSession.value.phase,
      ),
  );

  watch(
    () => selectedSession.value?.id,
    () => {
      if (renameDialogOpen.value) renameDialogOpen.value = false;
    },
    { flush: "sync" },
  );
  watch(
    editingSession,
    (session) => {
      if (!session && renameDialogOpen.value) renameDialogOpen.value = false;
    },
    { flush: "sync" },
  );

  watch(
    renameDialogOpen,
    (open) => {
      if (open) return;
      editGeneration += 1;
      cancelRenameSession();
      isRenamingSession.value = false;
    },
    { flush: "sync" },
  );

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
    editGeneration += 1;
    editingSessionId.value = selectedSession.value.id;
    renameDialogValue.value = selectedSession.value.title;
    initialPersistent = selectedSession.value.persistent;
    renameDialogPersistent.value = initialPersistent;
    renameDialogOpen.value = true;
  };

  const submitRenameDialog = async () => {
    const targetSession = editingSession.value;
    const nextTitle = renameDialogValue.value.trim();
    if (
      !renameDialogOpen.value ||
      isRenamingSession.value ||
      !targetSession ||
      !nextTitle
    )
      return;
    const generation = editGeneration;
    const persistent =
      !renameDialogPersistenceDisabled.value &&
      renameDialogPersistent.value !== initialPersistent
        ? renameDialogPersistent.value
        : undefined;

    isRenamingSession.value = true;
    try {
      await updateSessionTitle(targetSession.id, nextTitle, persistent);
      if (generation !== editGeneration || !renameDialogOpen.value) return;
      renameDialogOpen.value = false;
      focusTerminal();
    } catch (error) {
      if (generation !== editGeneration || !renameDialogOpen.value) return;
      toast.error(translate("admin.webTerminal.renameFailed"), {
        description: extractTerminalErrorMessage(
          error,
          translate("admin.webTerminal.renameFailedDescription"),
        ),
      });
    } finally {
      if (generation === editGeneration) isRenamingSession.value = false;
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
        description: extractTerminalErrorMessage(
          error,
          translate("admin.webTerminal.sendFailedDescription"),
        ),
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
    renameDialogPersistent,
    renameDialogPersistenceDisabled,
    sendDialogOpen,
    sendDialogPayload,
    submitRenameDialog,
    submitSendDialog,
  };
};
