import type { Ref } from "vue";
import { useI18n } from "vue-i18n";
import { GatewayLogsAPI } from "@/lib/api/gateway";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { getTodayString } from "./model";

export const useGatewayLogDates = ({
  selectedDate,
  availableDates,
  logsDir,
  currentRequest,
}: {
  selectedDate: Ref<string>;
  availableDates: Ref<string[]>;
  logsDir: Ref<string>;
  currentRequest: () => () => boolean;
}) => {
  const { t } = useI18n();
  const applyDates = (dates: string[], preferred?: string) => {
    const date = preferred || selectedDate.value || getTodayString();
    availableDates.value = [...new Set([...dates, date])].sort().reverse();
    selectedDate.value = date;
  };

  const fetchDates = async (preferred?: string) => {
    const isCurrent = currentRequest();
    try {
      const data = await GatewayLogsAPI.getDates();
      if (!isCurrent()) return;
      logsDir.value = data.logs_dir || "";
      applyDates(
        data.dates || [],
        preferred || data.today || selectedDate.value,
      );
    } catch (error) {
      if (isCurrent()) {
        toast.error(t("admin.gatewayRequestLogs.loadFailed"), {
          description: extractErrorMessage(error),
        });
        applyDates(availableDates.value, preferred);
      }
    }
  };

  return { applyDates, fetchDates };
};
