import { randomUUID } from "node:crypto";
import type { AcmeService } from "../plugins/acme/AcmeService";
import {
  configManager,
  type AcmeApplication,
  type AcmeJob,
  type AcmeJobTrigger,
  type AcmeRuntimeLock,
} from "./redis";
import { normalizeAcmeDnsType } from "./acme-dns-providers";
import { syncSSLDeploymentToGateway } from "./ssl-gateway";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
  type MessageParams,
  normalizeLocale,
  translate,
} from "../../../../packages/i18n/src";

const resolveAcmeJobLocale = (locale: string | null | undefined): LocaleCode =>
  normalizeLocale(locale) ?? DEFAULT_LOCALE;

const acmeJobT = (
  locale: string | null | undefined,
  key: string,
  params?: MessageParams,
) =>
  translate(
    resolveAcmeJobLocale(locale),
    `server.acmeJobRunner.${key}`,
    params,
  );

export const isAcmeJobTerminalStatus = (
  status: string | undefined | null,
): boolean =>
  status === "succeeded" || status === "failed" || status === "stopped";

const getManualStopMessage = (locale?: string | null) =>
  acmeJobT(locale, "manualStop");

const buildQueuedJob = (
  application: AcmeApplication,
  trigger: AcmeJobTrigger,
): AcmeJob => ({
  id: randomUUID(),
  applicationId: application.id,
  domains: application.domains,
  method: "dns",
  provider: normalizeAcmeDnsType(application.dnsType) || application.dnsType,
  trigger,
  createdAt: new Date().toISOString(),
  status: "queued",
  progress: 0,
  message: trigger === "auto_renew" ? "queued for renew" : "queued",
});

const getLockMessageByTrigger = (
  trigger: AcmeJobTrigger,
  locale?: string | null,
): string =>
  acmeJobT(
    locale,
    trigger === "auto_renew"
      ? "lockMessages.autoRenew"
      : "lockMessages.manualRequest",
  );

const normalizeDomainSet = (domains: string[]): string[] =>
  [
    ...new Set(
      domains.map((domain) => domain.trim().toLowerCase()).filter(Boolean),
    ),
  ].sort((a, b) => a.localeCompare(b));

const hasSameDomainSet = (left: string[], right: string[]): boolean => {
  const normalizedLeft = normalizeDomainSet(left);
  const normalizedRight = normalizeDomainSet(right);
  if (normalizedLeft.length !== normalizedRight.length) return false;
  return normalizedLeft.every(
    (domain, index) => domain === normalizedRight[index],
  );
};

const getAcmeLockHeartbeatIntervalMs = (): number =>
  Math.max(
    30_000,
    Math.min(
      60_000,
      Math.floor((configManager.getAcmeRuntimeLockTtlSeconds() * 1000) / 3),
    ),
  );

export const reserveAcmeApplicationJob = async (options: {
  application: AcmeApplication;
  trigger: AcmeJobTrigger;
  locale?: string | null;
}): Promise<{
  job: AcmeJob;
  lock: AcmeRuntimeLock;
}> => {
  const activeLock = await configManager.getActiveAcmeRuntimeLock();
  if (activeLock.locked) {
    throw new Error(acmeJobT(options.locale, "activeTaskRunning"));
  }

  const job = buildQueuedJob(options.application, options.trigger);
  const requestedLock: AcmeRuntimeLock = {
    locked: true,
    lockId: randomUUID(),
    jobId: job.id,
    applicationId: options.application.id,
    reason: options.trigger,
    startedAt: job.createdAt,
  };
  const lock = await configManager.tryAcquireAcmeRuntimeLock(requestedLock);
  if (!lock) {
    throw new Error(acmeJobT(options.locale, "activeTaskRunning"));
  }

  try {
    await configManager.createAcmeJob(job);
    await configManager.clearAcmeLogs(job.id);
    await configManager.updateAcmeApplicationJobState(
      options.application.id,
      job,
    );
  } catch (error) {
    await configManager.releaseAcmeRuntimeLock(lock);
    throw error;
  }

  return { job, lock };
};

export const failReservedAcmeApplicationJob = async (options: {
  applicationId: string;
  job: Pick<AcmeJob, "id" | "createdAt" | "trigger">;
  lock: AcmeRuntimeLock;
  message: string;
  locale?: string | null;
}): Promise<void> => {
  const finishedAt = new Date().toISOString();
  await configManager
    .appendAcmeLog(
      options.job.id,
      acmeJobT(options.locale, "flowFailed", { message: options.message }),
    )
    .catch(() => undefined);
  await configManager.updateAcmeJob(options.job.id, {
    applicationId: options.applicationId,
    status: "failed",
    progress: 100,
    finishedAt,
    message: options.message,
  });
  await configManager.updateAcmeApplicationJobState(options.applicationId, {
    id: options.job.id,
    status: "failed",
    trigger: options.job.trigger,
    createdAt: options.job.createdAt,
    finishedAt,
    message: options.message,
  });
  await configManager
    .releaseAcmeRuntimeLock(options.lock)
    .catch(() => undefined);
};

export const runReservedAcmeApplicationJob = async (options: {
  acme: AcmeService;
  application: AcmeApplication;
  trigger: AcmeJobTrigger;
  job: AcmeJob;
  lock: AcmeRuntimeLock;
  wait?: boolean;
  locale?: string | null;
}): Promise<{
  job: AcmeJob;
  lock: AcmeRuntimeLock;
}> => {
  await configManager.updateAcmeJob(options.job.id, {
    applicationId: options.application.id,
    domains: options.application.domains,
    provider:
      normalizeAcmeDnsType(options.application.dnsType) ||
      options.application.dnsType,
    trigger: options.trigger,
  });

  const task = executeAcmeApplicationJob({
    acme: options.acme,
    application: options.application,
    trigger: options.trigger,
    jobId: options.job.id,
    lock: options.lock,
    locale: options.locale,
  });

  if (options.wait) {
    await task;
  } else {
    void task;
  }

  return { job: options.job, lock: options.lock };
};

export const startAcmeApplicationJob = async (options: {
  acme: AcmeService;
  application: AcmeApplication;
  trigger: AcmeJobTrigger;
  wait?: boolean;
  locale?: string | null;
}): Promise<{
  job: AcmeJob;
  lock: AcmeRuntimeLock;
}> => {
  const reserved = await reserveAcmeApplicationJob({
    application: options.application,
    trigger: options.trigger,
    locale: options.locale,
  });

  try {
    return await runReservedAcmeApplicationJob({
      ...options,
      job: reserved.job,
      lock: reserved.lock,
      locale: options.locale,
    });
  } catch (error: any) {
    await failReservedAcmeApplicationJob({
      applicationId: options.application.id,
      job: reserved.job,
      lock: reserved.lock,
      message: error?.message || String(error),
      locale: options.locale,
    });
    throw error;
  }
};

export const stopActiveAcmeApplicationJob = async (options: {
  acme: AcmeService;
  message?: string;
  locale?: string | null;
}): Promise<{
  stopped: boolean;
  job: AcmeJob | null;
  lock: AcmeRuntimeLock;
  processResult: Awaited<ReturnType<AcmeService["stopAllAcmeProcesses"]>>;
}> => {
  const lock = await configManager.getActiveAcmeRuntimeLock();
  const message = options.message || getManualStopMessage(options.locale);
  const stoppedAt = new Date().toISOString();
  let job: AcmeJob | null = null;

  if (lock.locked && lock.jobId) {
    job = await configManager.getAcmeJob(lock.jobId);
    if (job && !isAcmeJobTerminalStatus(job.status)) {
      await configManager.appendAcmeLog(job.id, message).catch(() => undefined);
      await configManager.updateAcmeJob(job.id, {
        status: "stopped",
        progress: 100,
        finishedAt: stoppedAt,
        message,
      });
      if (job.applicationId) {
        await configManager.updateAcmeApplicationJobState(job.applicationId, {
          ...job,
          status: "stopped",
          finishedAt: stoppedAt,
          message,
        });
      }
      job = {
        ...job,
        status: "stopped",
        progress: 100,
        finishedAt: stoppedAt,
        message,
      };
    }
  }

  const processResult = await options.acme.stopAllAcmeProcesses();
  if (job) {
    const killedCount =
      processResult.matchedPids.length - processResult.remainingPids.length;
    await configManager
      .appendAcmeLog(
        job.id,
        processResult.matchedPids.length
          ? acmeJobT(options.locale, "stopSignalSent", {
              count: Math.max(0, killedCount),
            })
          : acmeJobT(options.locale, "noRunningProcess"),
      )
      .catch(() => undefined);
    for (const error of processResult.errors) {
      await configManager
        .appendAcmeLog(
          job.id,
          acmeJobT(options.locale, "stopProcessError", { message: error }),
        )
        .catch(() => undefined);
    }
    if (processResult.remainingPids.length > 0) {
      await configManager
        .appendAcmeLog(
          job.id,
          acmeJobT(options.locale, "processStillRunning", {
            pids: processResult.remainingPids.join(", "),
          }),
        )
        .catch(() => undefined);
    }
  }

  if (lock.locked && lock.lockId) {
    await configManager.releaseAcmeRuntimeLock(lock).catch(() => undefined);
  }

  return {
    stopped: Boolean(job),
    job,
    lock,
    processResult,
  };
};

export const executeAcmeApplicationJob = async (options: {
  acme: AcmeService;
  application: AcmeApplication;
  trigger: AcmeJobTrigger;
  jobId: string;
  lock: AcmeRuntimeLock;
  locale?: string | null;
}): Promise<void> => {
  const { acme, application, trigger, jobId } = options;
  const locale = options.locale;
  const startedAt = new Date().toISOString();
  let activeLock = options.lock;
  let heartbeatInFlight = false;
  let lockLossReason: string | null = null;
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  const lockLeaseTtlMs = configManager.getAcmeRuntimeLockTtlSeconds() * 1000;

  const persistJobPatch = async (patch: Partial<AcmeJob>) => {
    const currentJob = await configManager.getAcmeJob(jobId);
    if (currentJob?.status === "stopped" && patch.status !== "stopped") {
      throw new Error(getManualStopMessage(locale));
    }
    await configManager.updateAcmeJob(jobId, patch);
    const latestJob = await configManager.getAcmeJob(jobId);
    if (latestJob?.applicationId) {
      await configManager.updateAcmeApplicationJobState(
        latestJob.applicationId,
        latestJob,
      );
    }
  };

  const markLockLost = async (message: string) => {
    if (lockLossReason) return;
    lockLossReason = message;
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
    await configManager.appendAcmeLog(jobId, message).catch(() => undefined);
  };

  const ensureLockHealthy = () => {
    if (lockLossReason) {
      throw new Error(lockLossReason);
    }
  };

  const refreshLockLease = async () => {
    if (heartbeatInFlight || lockLossReason) return;
    heartbeatInFlight = true;
    try {
      const refreshed = await configManager.refreshAcmeRuntimeLock(activeLock);
      if (refreshed) {
        activeLock = refreshed;
        return;
      }
      await markLockLost(acmeJobT(locale, "lockLost"));
    } catch (error: any) {
      const message = acmeJobT(locale, "lockRefreshFailed", {
        message: error?.message || String(error),
      });
      const lastLeaseAtMs = Date.parse(
        activeLock.heartbeatAt || activeLock.startedAt || startedAt,
      );
      if (
        Number.isFinite(lastLeaseAtMs) &&
        Date.now() - lastLeaseAtMs >= lockLeaseTtlMs
      ) {
        await markLockLost(acmeJobT(locale, "lockLeaseExpired", { message }));
      } else {
        await configManager
          .appendAcmeLog(jobId, message)
          .catch(() => undefined);
      }
    } finally {
      heartbeatInFlight = false;
    }
  };

  heartbeatTimer = setInterval(() => {
    void refreshLockLease();
  }, getAcmeLockHeartbeatIntervalMs());
  heartbeatTimer.unref?.();

  try {
    await persistJobPatch({
      applicationId: application.id,
      trigger,
      status: "running",
      progress: 5,
      message: getLockMessageByTrigger(trigger, locale),
      startedAt,
    });

    ensureLockHealthy();
    const clientSettings = await configManager.ensureAcmeClientSettings(
      await acme.getDefaultCertificateAuthority(),
    );

    await acme.issueCertificate({
      domains: application.domains,
      method: "dns",
      dnsType: application.dnsType,
      certificateAuthority: clientSettings.certificateAuthority,
      envVars: application.credentials,
      onLog: async (line: string) => {
        ensureLockHealthy();
        await configManager.appendAcmeLog(jobId, line);
        ensureLockHealthy();
      },
    });

    ensureLockHealthy();
    await refreshLockLease();
    ensureLockHealthy();

    await persistJobPatch({
      progress: 80,
      message: "saving",
    });

    ensureLockHealthy();
    const latestApplication = await configManager.getAcmeApplication(
      application.id,
    );
    const applicationChanged =
      !latestApplication ||
      latestApplication.primaryDomain !== application.primaryDomain ||
      !hasSameDomainSet(latestApplication.domains, application.domains);
    const saved = applicationChanged
      ? false
      : await configManager.saveAcmeIssuedCertFromFS(
          application.id,
          application.primaryDomain,
          { forceInstall: true },
        );
    if (applicationChanged) {
      await configManager.appendAcmeLog(
        jobId,
        acmeJobT(locale, "applicationChangedSkipped"),
      );
    }
    if (!saved) {
      await configManager.appendAcmeLog(
        jobId,
        applicationChanged
          ? acmeJobT(locale, "issuedButApplicationChanged")
          : acmeJobT(locale, "issuedButCertReadFailed"),
      );
    }
    if (saved) {
      try {
        await acme.clearDomainWorkingState(application.primaryDomain);
        await configManager.appendAcmeLog(
          jobId,
          acmeJobT(locale, "clearedDomainWorkingState"),
        );
      } catch (error: any) {
        await configManager.appendAcmeLog(
          jobId,
          acmeJobT(locale, "clearDomainWorkingStateFailed", {
            message: error?.message || String(error),
          }),
        );
      }
    }

    const linkedLibraryCertificate = saved
      ? await configManager.getSSLCertificateBySourceRef("acme", application.id)
      : null;
    if (linkedLibraryCertificate) {
      const currentConfig = await configManager.getConfig();
      const shouldActivate =
        currentConfig.ssl.active_cert_id === linkedLibraryCertificate.id;
      await configManager.saveAcmeCertificateToLibraryByApplication(
        application.id,
        {
          id: linkedLibraryCertificate.id,
          label: linkedLibraryCertificate.label,
          activate: shouldActivate,
        },
      );

      if (shouldActivate || currentConfig.ssl.deployment_mode === "multi_sni") {
        await syncSSLDeploymentToGateway();
      }

      await configManager.appendAcmeLog(
        jobId,
        shouldActivate || currentConfig.ssl.deployment_mode === "multi_sni"
          ? acmeJobT(locale, "linkedLibrarySyncedGateway")
          : acmeJobT(locale, "linkedLibraryUpdated"),
      );
    } else if (saved) {
      try {
        const currentConfig = await configManager.getConfig();
        await configManager.saveAcmeCertificateToLibraryByApplication(
          application.id,
          {
            label:
              latestApplication?.name ||
              application.name ||
              application.primaryDomain,
          },
        );

        if (currentConfig.ssl.deployment_mode === "multi_sni") {
          await syncSSLDeploymentToGateway(currentConfig);
          await configManager.appendAcmeLog(
            jobId,
            acmeJobT(locale, "addedToLibraryAndSyncedGateway"),
          );
        } else {
          await configManager.appendAcmeLog(
            jobId,
            acmeJobT(locale, "addedToLibrary"),
          );
        }
      } catch (error: any) {
        await configManager.appendAcmeLog(
          jobId,
          acmeJobT(locale, "addToLibraryFailed", {
            message: error?.message || String(error),
          }),
        );
      }
    }

    await persistJobPatch({
      status: "succeeded",
      progress: 100,
      finishedAt: new Date().toISOString(),
      message: saved ? "succeeded" : "signed",
    });
  } catch (error: any) {
    const latestJob = await configManager.getAcmeJob(jobId).catch(() => null);
    if (latestJob?.status === "stopped") {
      await configManager
        .appendAcmeLog(jobId, acmeJobT(locale, "stoppedIgnoredProcessError"))
        .catch(() => undefined);
      return;
    }
    const message = error?.message || String(error);
    await configManager.appendAcmeLog(
      jobId,
      acmeJobT(locale, "flowFailed", { message }),
    );
    await persistJobPatch({
      status: "failed",
      progress: 100,
      finishedAt: new Date().toISOString(),
      message,
    });
  } finally {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
    }
    await configManager.releaseAcmeRuntimeLock(activeLock);
  }
};
