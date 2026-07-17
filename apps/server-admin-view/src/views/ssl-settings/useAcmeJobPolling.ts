import { onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { AcmeAPI, type AcmeJobData, type AcmeLogAnalysis } from "@/lib/api";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";

type UseAcmeJobPollingOptions = {
  refreshOverview: () => Promise<void>;
};

export function useAcmeJobPolling({
  refreshOverview,
}: UseAcmeJobPollingOptions) {
  const { t } = useI18n();
  const selectedJobId = ref("");
  const job = ref<AcmeJobData | null>(null);
  const logs = ref<string[]>([]);
  const analysis = ref<AcmeLogAnalysis | null>(null);
  let pollingTimer: ReturnType<typeof setInterval> | null = null;

  const { isPending: isRefreshingLogs, run: runRefreshLogs } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.acmeCert.refreshLogsFailed")),
      );
    },
  });
  const { isPending: isStoppingJob, run: runStopJob } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.acmeCert.stopJobFailed")),
      );
    },
  });

  const stopPolling = () => {
    if (!pollingTimer) return;
    clearInterval(pollingTimer);
    pollingTimer = null;
  };

  const pollJobOnce = async (jobId: string) => {
    const data = await AcmeAPI.poll(jobId, { limit: 500, order: "desc" });
    job.value = data.job;
    logs.value = data.logs;
    analysis.value = data.analysis ?? null;

    if (
      data.job.status === "succeeded" ||
      data.job.status === "failed" ||
      data.job.status === "stopped"
    ) {
      stopPolling();
      await refreshOverview();
    }
  };

  const startPolling = (jobId: string) => {
    stopPolling();
    pollingTimer = setInterval(async () => {
      try {
        await pollJobOnce(jobId);
      } catch {
        // Keep the last visible state and retry on the next interval.
      }
    }, 2000);
  };

  const selectJob = async (jobId: string, autoPoll: boolean) => {
    if (!jobId) return;
    selectedJobId.value = jobId;
    await pollJobOnce(jobId);
    if (
      autoPoll &&
      (job.value?.status === "queued" || job.value?.status === "running")
    ) {
      startPolling(jobId);
    } else {
      stopPolling();
    }
  };

  const viewJob = (jobId: string) => selectJob(jobId, false);

  const refreshLogs = async () => {
    if (!selectedJobId.value) return;
    await runRefreshLogs(() => pollJobOnce(selectedJobId.value));
  };

  const stopActiveJob = async () => {
    await runStopJob(async () => {
      const result = await AcmeAPI.stopActiveJob();
      stopPolling();
      const killedCount =
        result.processResult.matchedPids.length -
        result.processResult.remainingPids.length;
      if (result.stopped) {
        toast.success(t("admin.acmeCert.jobStopped"), {
          description:
            result.processResult.matchedPids.length > 0
              ? t("admin.acmeCert.jobStoppedDescription", {
                  count: Math.max(0, killedCount),
                })
              : t("admin.acmeCert.noRunningProcesses"),
        });
      } else {
        toast.info(t("admin.acmeCert.noActiveJob"));
      }

      await refreshOverview();
      const stoppedJobId = result.job?.id || selectedJobId.value;
      if (stoppedJobId) {
        await pollJobOnce(stoppedJobId);
      }
    });
  };

  const clearSelectedJob = (
    applicationId?: string,
    options: { includeRunning?: boolean } = {},
  ) => {
    if (applicationId && job.value?.applicationId !== applicationId) return;
    if (options.includeRunning === false && job.value?.status === "running") {
      return;
    }
    stopPolling();
    selectedJobId.value = "";
    job.value = null;
    logs.value = [];
    analysis.value = null;
  };

  onUnmounted(stopPolling);

  return {
    analysis,
    clearSelectedJob,
    isRefreshingLogs,
    isStoppingJob,
    job,
    logs,
    refreshLogs,
    selectJob,
    selectedJobId,
    stopActiveJob,
    viewJob,
  };
}
