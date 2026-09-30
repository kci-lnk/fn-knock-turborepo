<script setup lang="ts">
import TerminalCopyDialog from "./TerminalCopyDialog.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  copyDialogOpen,
  copyDialogText,
  copying,
  retryTerminalCopy,
  downloadCopyText,
  closeCopyDialog,
  focusAfterCopyDialog,
} = props.controller;
</script>

<template>
  <TerminalCopyDialog
    :open="copyDialogOpen"
    :text="copyDialogText"
    :copying="copying"
    @update:open="closeCopyDialog"
    @retry="retryTerminalCopy"
    @download="downloadCopyText"
    @close-auto-focus="focusAfterCopyDialog"
  />
</template>
