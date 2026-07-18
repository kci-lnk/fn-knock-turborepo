import { onScopeDispose, readonly, ref } from "vue";
import type { ButtonVariants } from "@/components/ui/button";

export interface ConfirmationDialogOptions {
  cancelText?: string;
  confirmText?: string;
  confirmVariant?: ButtonVariants["variant"];
  description: string;
  title: string;
}

const emptyOptions = (): ConfirmationDialogOptions => ({
  description: "",
  title: "",
});

export const useConfirmationDialog = () => {
  const confirmationDialogOpen = ref(false);
  const confirmationDialogOptions =
    ref<ConfirmationDialogOptions>(emptyOptions());
  let resolvePending: ((confirmed: boolean) => void) | undefined;

  const settleConfirmation = (confirmed: boolean) => {
    const resolve = resolvePending;
    resolvePending = undefined;
    confirmationDialogOpen.value = false;
    resolve?.(confirmed);
  };

  const requestConfirmation = (
    options: ConfirmationDialogOptions,
  ): Promise<boolean> => {
    resolvePending?.(false);
    confirmationDialogOptions.value = { ...options };
    confirmationDialogOpen.value = true;

    return new Promise((resolve) => {
      resolvePending = resolve;
    });
  };

  const handleConfirmationDialogOpenChange = (open: boolean) => {
    if (open) {
      confirmationDialogOpen.value = true;
      return;
    }
    settleConfirmation(false);
  };

  const confirmPendingAction = () => settleConfirmation(true);

  onScopeDispose(() => settleConfirmation(false));

  return {
    confirmationDialogOpen: readonly(confirmationDialogOpen),
    confirmationDialogOptions: readonly(confirmationDialogOptions),
    confirmPendingAction,
    handleConfirmationDialogOpenChange,
    requestConfirmation,
  };
};
