<template>
  <Dialog :open="open" @update:open="!loading && emit('update:open', $event)">
    <DialogContent
      class="max-h-[calc(100dvh-2rem)] overflow-y-auto break-words sm:max-w-[440px]"
      :show-close-button="!loading"
      @close-auto-focus="restoreOpenerFocus"
      @escape-key-down="loading && $event.preventDefault()"
      @interact-outside="loading && $event.preventDefault()"
    >
      <DialogHeader>
        <DialogTitle class="pr-6">{{ title }}</DialogTitle>
        <DialogDescription>
          {{ description }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button
          variant="outline"
          :disabled="loading"
          class="h-auto min-h-11 whitespace-normal sm:min-h-9"
          @click="emit('cancel')"
        >
          {{ cancelLabel }}
        </Button>
        <Button
          class="h-auto min-h-11 whitespace-normal sm:min-h-9"
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
import { watch } from "vue";
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

// These controlled dialogs can open from a portalled menu without a DialogTrigger.
// Preserve the menu's trigger, since the selected menu item is removed on close.
let opener: HTMLElement | null = null;
watch(
  () => props.open,
  (open) => {
    if (!open || typeof document === "undefined") return;
    const active = document.activeElement;
    const triggerId = active
      ?.closest('[role="menu"]')
      ?.getAttribute("aria-labelledby");
    opener = triggerId
      ? document.getElementById(triggerId)
      : active instanceof HTMLElement
        ? active
        : null;
  },
  { flush: "sync", immediate: true },
);
const restoreOpenerFocus = (event: Event) => {
  if (
    !opener?.isConnected ||
    opener === document.body ||
    opener.matches(":disabled")
  )
    return;
  event.preventDefault();
  opener.focus({ preventScroll: true });
};

const emit = defineEmits<{
  cancel: [];
  confirm: [];
  "update:open": [open: boolean];
}>();
</script>
