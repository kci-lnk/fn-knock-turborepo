import type { Ref } from "vue";
import { useI18n } from "vue-i18n";
import { ScannerAPI, type ScannerBlacklistRecord } from "@/lib/api/security";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";

type UseScannerFalsePositiveOptions = {
  detailRecord: Ref<ScannerBlacklistRecord | null>;
  isDetailsModalOpen: Ref<boolean>;
  clearSelection: () => void;
  fetchBlacklist: () => Promise<void>;
};

export function useScannerFalsePositive({
  detailRecord,
  isDetailsModalOpen,
  clearSelection,
  fetchBlacklist,
}: UseScannerFalsePositiveOptions) {
  const { t } = useI18n();
  const { isPending: isResolvingFalsePositive, run } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessions.ipBlacklist.falsePositiveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.ipBlacklist.falsePositiveFailedDescription"),
        ),
      });
    },
  });

  const resolveFalsePositive = async (path: string) => {
    const record = detailRecord.value;
    if (!record || isResolvingFalsePositive.value) return;
    await run(() => ScannerAPI.resolveFalsePositive({ ip: record.ip, path }), {
      onSuccess: async (result) => {
        toast.success(
          t(
            result.added
              ? "admin.sessions.ipBlacklist.falsePositiveResolved"
              : "admin.sessions.ipBlacklist.falsePositiveResolvedExisting",
            { path: result.path, ip: result.ip },
          ),
        );
        isDetailsModalOpen.value = false;
        detailRecord.value = null;
        clearSelection();
        await fetchBlacklist();
      },
    });
  };

  return { isResolvingFalsePositive, resolveFalsePositive };
}
