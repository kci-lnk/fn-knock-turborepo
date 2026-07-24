<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { LoaderCircle } from "lucide-vue-next";
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

defineProps<{
  disabled: boolean;
  onCloseAutoFocus: (event: Event) => void;
  open: boolean;
  payload: string;
  sending: boolean;
}>();

const emit = defineEmits<{
  submit: [];
  "update:open": [value: boolean];
  "update:payload": [value: string];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="sm:max-w-[560px]"
      @close-auto-focus="onCloseAutoFocus"
    >
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.webTerminal.sendDialogTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.webTerminal.sendDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <Textarea
        id="terminal-send-payload"
        :model-value="payload"
        class="min-h-[180px] font-mono text-sm"
        :placeholder="t('admin.webTerminal.sendDialogPlaceholder')"
        :disabled="disabled || sending"
        @update:model-value="emit('update:payload', String($event))"
        @keydown.ctrl.enter.prevent="emit('submit')"
      />

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="sending"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          :disabled="!payload.length || disabled || sending"
          @click="emit('submit')"
        >
          <LoaderCircle v-if="sending" class="mr-1.5 h-4 w-4 animate-spin" />
          {{ t("admin.webTerminal.send") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
