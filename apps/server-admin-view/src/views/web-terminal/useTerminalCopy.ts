import { ref, watch } from "vue";
import { toast } from "@admin-shared/utils/toast";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";

export function useTerminalCopy(translate: (key: string) => string) {
  const copyDialogOpen = ref(false);
  const copyDialogText = ref("");
  const copying = ref(false);
  let generation = 0;

  const invalidateCopy = () => {
    generation += 1;
    copying.value = false;
    copyDialogOpen.value = false;
    copyDialogText.value = "";
  };
  watch(
    copyDialogOpen,
    (open) => {
      if (!open) invalidateCopy();
    },
    { flush: "sync" },
  );

  const copyTerminalText = async (text: string) => {
    if (!text.trim()) {
      toast.info(translate("admin.webTerminal.noSelection"));
      return;
    }
    const current = ++generation;
    const origin = document.activeElement;
    const isCurrent = () => current === generation;
    const shouldContinue = () =>
      isCurrent() &&
      (document.activeElement === origin ||
        document.activeElement === document.body);
    const showText = () => {
      if (!isCurrent()) return;
      copyDialogText.value = text;
      copyDialogOpen.value = true;
    };
    copying.value = true;
    try {
      const result = await copyTextToClipboard(text, {
        strategy: "write-text-first",
        verify: false,
        shouldContinue,
      });
      if (!shouldContinue()) return;
      if (result.method === "clipboard.writeText") {
        toast.success(translate("admin.webTerminal.copyCompleted"));
      } else {
        toast.info(translate("admin.webTerminal.copyAttempted"), {
          action: {
            label: translate("admin.webTerminal.viewCopyText"),
            onClick: showText,
          },
        });
      }
    } catch {
      if (!shouldContinue()) return;
      toast.error(translate("admin.webTerminal.copyFailed"));
      showText();
    } finally {
      if (isCurrent()) copying.value = false;
    }
  };

  const retryTerminalCopy = () => copyTerminalText(copyDialogText.value);
  const downloadCopyText = () => {
    const url = URL.createObjectURL(
      new Blob([copyDialogText.value], { type: "text/plain;charset=utf-8" }),
    );
    const link = document.createElement("a");
    link.href = url;
    link.download = "terminal-logs.txt";
    link.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  };

  return {
    copyDialogOpen,
    copyDialogText,
    copying,
    copyTerminalText,
    retryTerminalCopy,
    downloadCopyText,
    invalidateCopy,
  };
}
