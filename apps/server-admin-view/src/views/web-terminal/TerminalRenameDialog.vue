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
import { Input } from "@/components/ui/input";

defineProps<{
  open: boolean;
  renaming: boolean;
  value: string;
}>();

const emit = defineEmits<{
  submit: [];
  "update:open": [value: boolean];
  "update:value": [value: string];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.webTerminal.renameSession") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.webTerminal.renameDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <Input
        :model-value="value"
        :placeholder="t('admin.webTerminal.renameDialogPlaceholder')"
        :disabled="renaming"
        @update:model-value="emit('update:value', String($event))"
        @keydown.enter.prevent="emit('submit')"
      />

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="renaming"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="!value.trim().length || renaming" @click="emit('submit')">
          <LoaderCircle v-if="renaming" class="mr-1.5 h-4 w-4 animate-spin" />
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
