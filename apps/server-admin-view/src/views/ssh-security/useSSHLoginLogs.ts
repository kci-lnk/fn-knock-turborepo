import { computed, onUnmounted, ref, watch } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import type { SSHLoginLogEntry } from "@/types";

type LoginLogOutcome = "all" | "success" | "failure";
type Translate = (key: string) => string;

export const useSSHLoginLogs = ({
  fetchLogs,
  translate,
}: {
  fetchLogs: (params: {
    limit: string;
    outcome: LoginLogOutcome;
    page: number;
    search: string;
  }) => Promise<{ items: SSHLoginLogEntry[]; total: number }>;
  translate: Translate;
}) => {
  const logItems = ref<SSHLoginLogEntry[]>([]);
  const logTotal = ref(0);
  const logPage = ref(1);
  const logLimit = ref("20");
  const logSearch = ref("");
  const logOutcome = ref<LoginLogOutcome>("all");
  let logSearchTimer: ReturnType<typeof setTimeout> | null = null;

  const { isPending: isLoadingLogs, run: runLoadLogs } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.sshSecurity.logsLoadFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.sshSecurity.logsLoadDescription"),
        ),
      });
    },
  });

  const logParsedLimit = computed(() => {
    const parsed = Number.parseInt(logLimit.value, 10);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 20;
  });

  const loadLogs = async () => {
    await runLoadLogs(
      () =>
        fetchLogs({
          page: logPage.value,
          limit: logLimit.value,
          search: logSearch.value,
          outcome: logOutcome.value,
        }),
      {
        onSuccess: (payload) => {
          logItems.value = payload.items;
          logTotal.value = payload.total;
        },
      },
    );
  };

  const handleLogSearch = () => {
    logPage.value = 1;
    void loadLogs();
  };

  const handleLogPageChange = (page: number) => {
    logPage.value = page;
    void loadLogs();
  };

  const handleLogLimitChange = (value: unknown) => {
    logLimit.value = String(value ?? "20");
    logPage.value = 1;
    void loadLogs();
  };

  watch(logSearch, () => {
    if (logSearchTimer) clearTimeout(logSearchTimer);
    logSearchTimer = setTimeout(handleLogSearch, 500);
  });

  watch(logOutcome, () => {
    handleLogSearch();
  });

  onUnmounted(() => {
    if (logSearchTimer) {
      clearTimeout(logSearchTimer);
    }
  });

  return {
    handleLogLimitChange,
    handleLogPageChange,
    handleLogSearch,
    isLoadingLogs,
    loadLogs,
    logItems,
    logLimit,
    logOutcome,
    logPage,
    logParsedLimit,
    logSearch,
    logTotal,
  };
};
