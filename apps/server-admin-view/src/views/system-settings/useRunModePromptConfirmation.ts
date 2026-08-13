import { ref } from "vue";
import { SystemAPI, type RunModePromptPreferences } from "@/lib/api/system";
import type { ReverseProxySubmode } from "@/types";

type RunMode = 0 | 1 | 3;
type PromptKey = keyof RunModePromptPreferences;
type ApplyRunModeChange = (
  mode: RunMode,
  submode: ReverseProxySubmode | null,
  options: {
    promptPreferenceKey: PromptKey | null;
    disablePrompt: boolean;
    onSuccess: () => void;
  },
) => Promise<void>;

const getPromptPreferenceKey = (
  currentMode: RunMode,
  nextMode: RunMode,
): PromptKey | null => {
  if (currentMode === 0 && nextMode === 1) return "directToReverseProxy";
  if (currentMode === 1 && nextMode === 0) return "reverseProxyToDirect";
  if (nextMode === 3) return "switchToSubdomain";
  if (currentMode === 3 && nextMode === 1) return "subdomainToReverseProxy";
  return null;
};

export const useRunModePromptConfirmation = () => {
  const pendingMode = ref<RunMode | null>(null);
  const pendingSubmode = ref<ReverseProxySubmode | null>(null);
  const pendingPromptKey = ref<PromptKey | null>(null);
  const isConfirmDialogOpen = ref(false);
  const dontShowAgainChecked = ref(false);
  const runModePromptPreferences = ref<RunModePromptPreferences>({
    directToReverseProxy: false,
    reverseProxyToDirect: false,
    switchToSubdomain: false,
    subdomainToReverseProxy: false,
  });

  const resetConfirmation = () => {
    pendingMode.value = null;
    pendingSubmode.value = null;
    pendingPromptKey.value = null;
    dontShowAgainChecked.value = false;
  };

  const closeConfirmation = () => {
    isConfirmDialogOpen.value = false;
    resetConfirmation();
  };

  const handleConfirmDialogOpenChange = (nextOpen: boolean) => {
    isConfirmDialogOpen.value = nextOpen;
    if (!nextOpen) {
      resetConfirmation();
    }
  };

  const queueConfirmation = ({
    currentMode,
    nextMode,
    nextSubmode,
  }: {
    currentMode: RunMode;
    nextMode: RunMode;
    nextSubmode: ReverseProxySubmode;
  }) => {
    const promptKey = getPromptPreferenceKey(currentMode, nextMode);
    if (!promptKey || runModePromptPreferences.value[promptKey]) {
      return false;
    }

    pendingMode.value = nextMode;
    pendingSubmode.value = nextMode === 1 ? nextSubmode : null;
    pendingPromptKey.value = promptKey;
    dontShowAgainChecked.value = false;
    isConfirmDialogOpen.value = true;
    return true;
  };

  const confirm = async (applyRunModeChange: ApplyRunModeChange) => {
    if (pendingMode.value === null) return;
    const nextMode = pendingMode.value;
    const nextSubmode = pendingSubmode.value;

    await applyRunModeChange(nextMode, nextMode === 1 ? nextSubmode : null, {
      promptPreferenceKey: pendingPromptKey.value,
      disablePrompt: dontShowAgainChecked.value,
      onSuccess: closeConfirmation,
    });
  };

  const loadRunModePromptPreferences = async () => {
    try {
      runModePromptPreferences.value =
        await SystemAPI.getRunModePromptPreferences();
    } catch (error) {
      console.warn("load run mode prompt preferences failed:", error);
    }
  };

  return {
    closeConfirmation,
    confirm,
    dontShowAgainChecked,
    handleConfirmDialogOpenChange,
    isConfirmDialogOpen,
    loadRunModePromptPreferences,
    pendingMode,
    pendingPromptKey,
    pendingSubmode,
    queueConfirmation,
    runModePromptPreferences,
  };
};
