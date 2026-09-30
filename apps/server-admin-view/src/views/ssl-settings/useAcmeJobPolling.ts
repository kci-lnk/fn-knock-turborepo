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
import { notifyAcmeJobStop } from "./acme-job-feedback";

type UseAcmeJobPollingOptions = {
  refreshOverview: () => Promise<void>;
  isRuntimeLocked?: () => boolean;
};

export function useAcmeJobPolling({
  refreshOverview,
  isRuntimeLocked = () => false,
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

  const pollJobOnce = async (
    jobId: string,
    signal?: AbortSignal,
    refreshOnCompletion = true,
  ) => {
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
      if (refreshOnCompletion) await refreshOverview();
      // A stopped job may still be finishing its file transaction. Keep the
      // normal interval until the runtime lock is released and actions unlock.
      if (!isRuntimeLocked()) stopPolling();
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
    // Overview loading may select a terminal job while its executor still
    // holds the runtime lock. Do not recurse back into overview loading here.
    await pollJobOnce(jobId, undefined, false);
    if (
      autoPoll &&
      (job.value?.status === "queued" ||
        job.value?.status === "running" ||
        isRuntimeLocked())
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
      notifyAcmeJobStop(result, (key, params) =>
        params ? t(key, params) : t(key),
      );

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
