<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Button, type ButtonVariants } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const props = withDefaults(
  defineProps<{
    cancelText?: string;
    confirmText?: string;
    confirmVariant?: ButtonVariants["variant"];
    description: string;
    open: boolean;
    title: string;
  }>(),
  {
    confirmVariant: "default",
  },
);

const emit = defineEmits<{
  confirm: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
const cancelText = computed(() => props.cancelText ?? t("common.cancel"));
const confirmText = computed(() => props.confirmText ?? t("common.confirm"));
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[440px]">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="emit('update:open', false)">
          {{ cancelText }}
        </Button>
        <Button :variant="confirmVariant" @click="emit('confirm')">
          {{ confirmText }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
