import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { MaintenanceAPI } from "@/lib/api/config";

export const useMaintenanceClearData = () => {
  const { t } = useI18n();
  const isClearDataDialogOpen = ref(false);
  const clearDataConfirmation = ref("");
  const { isPending: isClearingData, run: runClearData } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.maintenanceSettings.clearAllDataFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.maintenanceSettings.clearAllDataFailedDescription"),
        ),
      });
    },
  });

  const expectedClearDataConfirmation = computed(() =>
    t("admin.maintenanceSettings.clearAllDataConfirmationPhrase"),
  );
  const canClearAllData = computed(
    () =>
      !isClearingData.value &&
      clearDataConfirmation.value === expectedClearDataConfirmation.value,
  );

  const openClearDataDialog = () => {
    if (isClearingData.value) return;
    clearDataConfirmation.value = "";
    isClearDataDialogOpen.value = true;
  };

  const handleClearDataDialogOpenChange = (open: boolean) => {
    if (isClearingData.value) return;
    isClearDataDialogOpen.value = open;
    if (!open) clearDataConfirmation.value = "";
  };

  const clearAllData = async () => {
    if (!canClearAllData.value) return;
    await runClearData(
      () => MaintenanceAPI.clearAllData(clearDataConfirmation.value),
      {
        onSuccess: () => {
          if (typeof window === "undefined") return;
          window.localStorage.clear();
          window.location.reload();
        },
      },
    );
  };

  const handleClearDataEnter = () => {
    if (canClearAllData.value) void clearAllData();
  };

  return {
    canClearAllData,
    clearAllData,
    clearDataConfirmation,
    expectedClearDataConfirmation,
    handleClearDataDialogOpenChange,
    handleClearDataEnter,
    isClearDataDialogOpen,
    isClearingData,
    openClearDataDialog,
  };
};
