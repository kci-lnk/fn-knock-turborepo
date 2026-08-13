<script setup lang="ts">
import TerminalRenameDialog from "./TerminalRenameDialog.vue";
import TerminalSendDialog from "./TerminalSendDialog.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  focusTerminalAfterDialogClose,
  isRenamingSession,
  isSendingDialogPayload,
  renameDialogOpen,
  renameDialogValue,
  sendDialogOpen,
  sendDialogPayload,
  submitRenameDialog,
  submitSendDialog,
  toolbarDisabled,
} = props.controller;
</script>

<template>
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
</template>
