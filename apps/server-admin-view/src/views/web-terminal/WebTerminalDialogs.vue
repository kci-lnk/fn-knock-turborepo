<script setup lang="ts">
import TerminalRenameDialog from "./TerminalRenameDialog.vue";
import TerminalLocalSettingsDialog from "./TerminalLocalSettingsDialog.vue";
import TerminalSendDialog from "./TerminalSendDialog.vue";
import TerminalTargetEditorDialog from "./TerminalTargetEditorDialog.vue";
import TerminalTargetForceDeleteDialog from "./TerminalTargetForceDeleteDialog.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  focusTerminalAfterDialogClose,
  canConfirmForceDelete,
  confirmForceDeleteTarget,
  closeForceDeleteTarget,
  forceDeletingTarget,
  deleteTarget,
  pendingForceDeleteActiveCount,
  pendingForceDeleteMessage,
  pendingForceDeleteTarget,
  isRenamingSession,
  isSendingDialogPayload,
  renameDialogOpen,
  renameDialogValue,
  selectedTargetActiveSessionCount,
  sendDialogOpen,
  sendDialogPayload,
  submitRenameDialog,
  submitSendDialog,
  targetEditor,
  toolbarDisabled,
} = props.controller;
</script>

<template>
  <TerminalLocalSettingsDialog :controller="controller" />

  <TerminalSendDialog
    v-model:open="sendDialogOpen"
    v-model:payload="sendDialogPayload"
    :disabled="toolbarDisabled"
    :on-close-auto-focus="focusTerminalAfterDialogClose"
    :sending="isSendingDialogPayload"
    @submit="submitSendDialog"
  />

  <TerminalRenameDialog
    v-model:open="renameDialogOpen"
    v-model:value="renameDialogValue"
    :renaming="isRenamingSession"
    @submit="submitRenameDialog"
  />

  <TerminalTargetEditorDialog
    :active-session-count="selectedTargetActiveSessionCount"
    :editor="targetEditor"
    :on-delete="deleteTarget"
  />

  <TerminalTargetForceDeleteDialog
    :active-session-count="pendingForceDeleteActiveCount"
    :can-confirm="canConfirmForceDelete"
    :deleting="forceDeletingTarget"
    :message="pendingForceDeleteMessage"
    :open="!!pendingForceDeleteTarget"
    :target-name="pendingForceDeleteTarget?.name || ''"
    @close="closeForceDeleteTarget"
    @confirm="confirmForceDeleteTarget"
  />
</template>
