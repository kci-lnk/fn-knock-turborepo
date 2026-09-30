<script setup lang="ts">
import { useId } from "vue";
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
import { Label } from "@/components/ui/label";

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
const fieldId = useId();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-[480px]"
    >
      <DialogHeader>
        <DialogTitle>{{ t("admin.webTerminal.renameSession") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.webTerminal.renameDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <form class="space-y-5" @submit.prevent="emit('submit')">
        <div class="space-y-2">
          <Label :for="`${fieldId}-name`">{{ t("common.name") }}</Label>
          <Input
            :id="`${fieldId}-name`"
            :aria-label="t('admin.webTerminal.renameDialogPlaceholder')"
            :model-value="value"
            :placeholder="t('admin.webTerminal.renameDialogPlaceholder')"
            :disabled="renaming"
            @update:model-value="emit('update:value', String($event))"
          />
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            :disabled="renaming"
            @click="emit('update:open', false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button type="submit" :disabled="!value.trim().length || renaming">
            <LoaderCircle v-if="renaming" class="mr-1.5 h-4 w-4 animate-spin" />
            {{ t("common.save") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
