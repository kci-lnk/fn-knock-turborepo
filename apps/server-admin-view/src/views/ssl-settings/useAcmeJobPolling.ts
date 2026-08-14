import { onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  AcmeAPI,
  type AcmeJobData,
  type AcmeLogAnalysis,
} from "@/lib/api/acme";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";

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
  let isDisposed = false;

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

  const pollJobOnce = async (jobId: string, signal?: AbortSignal) => {
    const data = await AcmeAPI.poll(jobId, {
      limit: 500,
      order: "desc",
      signal,
    });
    if (signal?.aborted || selectedJobId.value !== jobId) return;

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
    if (isDisposed) return;
    selectedJobId.value = jobId;
    jobPoller.start();
    jobPoller.sync();
  };

  const jobPoller = createVisibilityPoller({
    intervalMs: 2_000,
    task: async (signal) => {
      if (!selectedJobId.value) return;
      try {
        await pollJobOnce(selectedJobId.value, signal);
      } catch {
        // Keep the last visible state and retry on the next interval.
      }
    },
  });

  const stopPolling = jobPoller.stop;

  const selectJob = async (jobId: string, autoPoll: boolean) => {
    if (!jobId) return;
    stopPolling();
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
      const stopErrors = result.processResult.errors;
      const remainingPids = result.processResult.remainingPids;
      if (
        !result.stopped &&
        (Boolean(result.job) || stopErrors.length > 0 || remainingPids.length > 0)
      ) {
        const details = [
          ...stopErrors,
          ...(remainingPids.length
            ? [`PID: ${remainingPids.join(", ")}`]
            : []),
        ].join("; ");
        toast.error(t("admin.acmeCert.stopJobFailed"), {
          description: details || undefined,
        });
      } else if (result.stopped) {
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

  onUnmounted(() => {
    isDisposed = true;
    stopPolling();
  });

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
