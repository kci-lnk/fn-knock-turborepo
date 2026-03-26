import { randomUUID } from "node:crypto";
import type { AcmeService } from "../plugins/acme/AcmeService";
import {
  configManager,
  type AcmeApplication,
  type AcmeJob,
  type AcmeJobTrigger,
  type AcmeRuntimeLock,
} from "./redis";
import { syncSSLDeploymentToGateway } from "./ssl-gateway";

export const isAcmeJobTerminalStatus = (
  status: string | undefined | null,
): boolean => status === "succeeded" || status === "failed";

const buildQueuedJob = (
  application: AcmeApplication,
  trigger: AcmeJobTrigger,
): AcmeJob => ({
  id: randomUUID(),
  applicationId: application.id,
  domains: application.domains,
  method: "dns",
  provider: application.dnsType,
  trigger,
  createdAt: new Date().toISOString(),
  status: "queued",
  progress: 0,
  message: trigger === "auto_renew" ? "queued for renew" : "queued",
});

const lockMessageByTrigger: Record<AcmeJobTrigger, string> = {
  manual_request: "正在申请证书",
  auto_renew: "正在自动续期证书",
};

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
}): Promise<{
  job: AcmeJob;
  lock: AcmeRuntimeLock;
}> => {
  const activeLock = await configManager.getActiveAcmeRuntimeLock();
  if (activeLock.locked) {
    throw new Error("当前已有 ACME 任务正在执行，请稍后再试");
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
    throw new Error("当前已有 ACME 任务正在执行，请稍后再试");
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
}): Promise<void> => {
  const finishedAt = new Date().toISOString();
  await configManager
    .appendAcmeLog(options.job.id, `证书申请流程失败: ${options.message}`)
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
}): Promise<{
  job: AcmeJob;
  lock: AcmeRuntimeLock;
}> => {
  await configManager.updateAcmeJob(options.job.id, {
    applicationId: options.application.id,
    domains: options.application.domains,
    provider: options.application.dnsType,
    trigger: options.trigger,
  });

  const task = executeAcmeApplicationJob({
    acme: options.acme,
    application: options.application,
    trigger: options.trigger,
    jobId: options.job.id,
    lock: options.lock,
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
}): Promise<{
  job: AcmeJob;
  lock: AcmeRuntimeLock;
}> => {
  const reserved = await reserveAcmeApplicationJob({
    application: options.application,
    trigger: options.trigger,
  });

  try {
    return await runReservedAcmeApplicationJob({
      ...options,
      job: reserved.job,
      lock: reserved.lock,
    });
  } catch (error: any) {
    await failReservedAcmeApplicationJob({
      applicationId: options.application.id,
      job: reserved.job,
      lock: reserved.lock,
      message: error?.message || String(error),
    });
    throw error;
  }
};

export const executeAcmeApplicationJob = async (options: {
  acme: AcmeService;
  application: AcmeApplication;
  trigger: AcmeJobTrigger;
  jobId: string;
  lock: AcmeRuntimeLock;
}): Promise<void> => {
  const { acme, application, trigger, jobId } = options;
  const startedAt = new Date().toISOString();
  let activeLock = options.lock;
  let heartbeatInFlight = false;
  let lockLossReason: string | null = null;
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  const lockLeaseTtlMs = configManager.getAcmeRuntimeLockTtlSeconds() * 1000;

  const persistJobPatch = async (patch: Partial<AcmeJob>) => {
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
      await markLockLost("ACME 运行锁已丢失，任务已停止，请重新发起申请");
    } catch (error: any) {
      const message = `ACME 运行锁续租异常: ${error?.message || String(error)}`;
      const lastLeaseAtMs = Date.parse(
        activeLock.heartbeatAt || activeLock.startedAt || startedAt,
      );
      if (
        Number.isFinite(lastLeaseAtMs) &&
        Date.now() - lastLeaseAtMs >= lockLeaseTtlMs
      ) {
        await markLockLost(
          `${message}；已超过锁租期，任务已停止，请重新发起申请`,
        );
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

  await persistJobPatch({
    applicationId: application.id,
    trigger,
    status: "running",
    progress: 5,
    message: lockMessageByTrigger[trigger],
    startedAt,
  });

  try {
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
        "申请项域名已在执行期间发生变化，已跳过写入旧证书，请重新发起申请",
      );
    }
    if (!saved) {
      await configManager.appendAcmeLog(
        jobId,
        applicationChanged
          ? "证书签发成功，但由于申请项域名已变更，未写入当前申请项"
          : "证书签发成功，但读取证书文件失败（请稍后重试或检查 acme.sh 目录）",
      );
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
          ? "已同步已关联的证书库条目，并刷新网关证书列表"
          : "已更新已关联的证书库条目",
      );
    }

    await persistJobPatch({
      status: "succeeded",
      progress: 100,
      finishedAt: new Date().toISOString(),
      message: saved ? "succeeded" : "signed",
    });
  } catch (error: any) {
    const message = error?.message || String(error);
    await configManager.appendAcmeLog(jobId, `证书申请流程失败: ${message}`);
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
