import type { AcmeAPI } from "@/lib/api/acme";
import { toast } from "@admin-shared/utils/toast";

export function notifyAcmeJobStop(
  result: Awaited<ReturnType<typeof AcmeAPI.stopActiveJob>>,
  t: (key: string, params?: Record<string, unknown>) => string,
) {
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
      ...(remainingPids.length ? [`PID: ${remainingPids.join(", ")}`] : []),
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
}
