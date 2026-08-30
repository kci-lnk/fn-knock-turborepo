import { nextTick, watch, type Ref } from "vue";

export const useDialogFocusRestore = (isOpen: Ref<boolean>) => {
  let trigger: HTMLElement | null = null;

  const openDialog = (event: MouseEvent) => {
    if (event.currentTarget instanceof HTMLElement) {
      trigger = event.currentTarget;
    }
    isOpen.value = true;
  };

  watch(isOpen, (open) => {
    if (open) return;
    const target = trigger;
    trigger = null;
    void nextTick(() => {
      if (target?.isConnected) {
        target.focus({ preventScroll: true });
      }
    });
  });

  return { openDialog };
};
