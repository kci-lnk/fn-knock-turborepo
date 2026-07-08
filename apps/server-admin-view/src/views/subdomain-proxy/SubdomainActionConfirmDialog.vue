<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[440px]">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>
          {{ description }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="emit('cancel')">
          {{ cancelLabel }}
        </Button>
        <Button
          :variant="confirmVariant"
          :disabled="loading"
          @click="emit('confirm')"
        >
          <span
            v-if="loading"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ confirmLabel }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { Button, type ButtonVariants } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

withDefaults(
  defineProps<{
    cancelLabel: string;
    confirmLabel: string;
    confirmVariant?: ButtonVariants["variant"];
    description: string;
    loading: boolean;
    open: boolean;
    title: string;
  }>(),
  {
    confirmVariant: "default",
  },
);

const emit = defineEmits<{
  cancel: [];
  confirm: [];
  "update:open": [open: boolean];
}>();
</script>
