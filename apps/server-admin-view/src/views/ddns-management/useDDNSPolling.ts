import { computed, ref, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  DEFAULT_LOG_WINDOW_SIZE,
  mergePollingLogWindow,
} from "@admin-shared/utils/log-window";
import type { DDNSStatusPayload } from "@/lib/api";
import { DDNSAPI } from "@/lib/api";
import { useTargetPolling } from "@/composables/useTargetPolling";
import type { LogEntry } from "./model";

export const useDDNSPolling = ({
  applyStatus,
  enabled,
  isPrimaryConfigDirty,
}: {
  applyStatus: (
    status: DDNSStatusPayload,
    options?: { syncEnabled?: boolean; syncProvider?: boolean },
  ) => void;
  enabled: Ref<boolean>;
  isPrimaryConfigDirty: Readonly<Ref<boolean>>;
}) => {
  const { locale, t } = useI18n();
  const logs = ref<LogEntry[]>([]);
  let enabledInitialized = false;

  const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
    onError: () => {
      toast.error(t("admin.ddns.clearLogsFailed"));
    },
  });
  const { isPending: isTogglingEnabled, run: runToggleEnabled } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.ddns.toggleFailed"), {
          description: extractErrorMessage(error, t("admin.ddns.toggleFailed")),
        });
      },
    });

  const polling = useTargetPolling({
    target: "ddns",
    intervalMs: 2000,
    onData: (payload) => {
      logs.value = mergePollingLogWindow(
        logs.value,
        payload.logs as LogEntry[],
        {
          reset: payload.reset,
          max: DEFAULT_LOG_WINDOW_SIZE,
        },
      );

      const status = payload.status;
      applyStatus(status, {
        syncEnabled: false,
        syncProvider: !isPrimaryConfigDirty.value,
      });
      if (enabledInitialized && status.enabled !== enabled.value) {
        enabledInitialized = false;
        enabled.value = status.enabled;
        enabledInitialized = true;
      }
    },
    onError: (error) => {
      console.error(
        "ddns poll:",
        extractErrorMessage(error, t("admin.ddns.pollStatusFailed")),
      );
    },
  });

  watch(enabled, async (value) => {
    if (!enabledInitialized) return;
    await runToggleEnabled(() => DDNSAPI.toggle(value), {
      onSuccess: () => {
        toast.success(
          value ? t("admin.ddns.enabled") : t("admin.ddns.disabled"),
        );
      },
      onError: () => {
        enabledInitialized = false;
        enabled.value = !value;
        enabledInitialized = true;
      },
    });
  });

  const refresh = () => {
    polling.resetCursor();
    void polling.refresh();
  };

  const start = () => {
    enabledInitialized = true;
    polling.start();
  };

  const stop = () => {
    enabledInitialized = false;
    polling.stop();
  };

  const onClearLogs = async () => {
    await runClearLogs(() => DDNSAPI.clearLogs(), {
      onSuccess: () => {
        logs.value = [];
        refresh();
        toast.success(t("admin.ddns.logsCleared"));
      },
    });
  };

  const logLines = computed(() =>
    logs.value.map((entry) => {
      const tag =
        entry.level === "error"
          ? t("admin.ddns.logLevelError")
          : entry.level === "warn"
            ? t("admin.ddns.logLevelWarn")
            : t("admin.ddns.logLevelInfo");
      const time = formatDateTimeSafe(entry.time, {
        locale: String(locale.value),
        emptyText: t("admin.ddns.never"),
      });
      return `${tag} ${time}  ${entry.message}`;
    }),
  );

  return {
    isClearingLogs,
    isTogglingEnabled,
    logLines,
    logs,
    onClearLogs,
    refresh,
    start,
    stop,
  };
};
