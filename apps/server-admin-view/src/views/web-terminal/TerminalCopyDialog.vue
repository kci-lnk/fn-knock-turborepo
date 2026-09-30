<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";

defineProps<{ open: boolean; text: string; copying: boolean }>();
const emit = defineEmits<{
  "update:open": [value: boolean];
  "close-auto-focus": [event: Event];
  retry: [];
  download: [];
}>();
const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="sm:max-w-[720px]"
      @close-auto-focus="emit('close-auto-focus', $event)"
    >
      <DialogHeader>
        <DialogTitle>{{ t("admin.webTerminal.copyDialogTitle") }}</DialogTitle>
        <DialogDescription>{{
          t("admin.webTerminal.copyDialogDescription")
        }}</DialogDescription>
      </DialogHeader>
      <Textarea
        :model-value="text"
        :aria-label="t('admin.webTerminal.copyDialogTitle')"
        readonly
        class="h-[min(50vh,400px)] font-mono text-sm"
      />
      <DialogFooter>
        <Button variant="outline" @click="emit('update:open', false)">{{
          t("shared.detailDialog.close")
        }}</Button>
        <Button variant="outline" @click="emit('download')">{{
          t("admin.webTerminal.downloadCopyText")
        }}</Button>
        <Button :disabled="copying" @click="emit('retry')">{{
          t("admin.webTerminal.retryCopy")
        }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
