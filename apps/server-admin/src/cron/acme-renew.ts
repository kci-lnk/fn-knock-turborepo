import type { Elysia } from "elysia";
import { cron } from "@elysiajs/cron";
import { acmeService } from "../plugins/acme";
import { configManager } from "../lib/redis";
import { startAcmeApplicationJob } from "../lib/acme-job-runner";
import { tDefault } from "../lib/i18n";
import { syncSSLDeploymentToGateway } from "../lib/ssl-gateway";
import {
  DEFAULT_LOCALE,
  normalizeLocale,
  translate,
} from "../../../../packages/i18n/src";

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

const isExpiringSoon = (validTo: string | undefined, thresholdMs: number) => {
  if (!validTo) return false;
  const t = Date.parse(validTo);
  if (!Number.isFinite(t)) return false;
  return t - Date.now() <= thresholdMs;
};

const activeAcmeTaskMessage = (locale?: string | null) =>
  translate(
    normalizeLocale(locale) ?? DEFAULT_LOCALE,
    "server.acmeJobRunner.activeTaskRunning",
  );

const samePem = (left: string | undefined, right: string | undefined) =>
  (left || "").trim() === (right || "").trim();

const reconcileAcmeSslDeployment = async () => {
  try {
    const applications = await configManager.listAcmeApplications();
    let config = await configManager.getConfig();
    let deploymentChanged = false;

    for (const application of applications) {
      if (!application.renewEnabled) continue;

      try {
        const issuedCertificate =
          await configManager.getUsableAcmeIssuedCertificate(application.id);
        if (!issuedCertificate) continue;

        const linkedLibraryCertificate =
          await configManager.getSSLCertificateBySourceRef(
            "acme",
            application.id,
          );
        const libraryMatchesIssued =
          linkedLibraryCertificate &&
          samePem(linkedLibraryCertificate.cert, issuedCertificate.cert) &&
          samePem(linkedLibraryCertificate.key, issuedCertificate.key);

        if (libraryMatchesIssued) continue;

        const shouldActivate =
          !!linkedLibraryCertificate &&
          config.ssl.active_cert_id === linkedLibraryCertificate.id;

        await configManager.saveAcmeCertificateToLibraryByApplication(
          application.id,
          {
            id: linkedLibraryCertificate?.id,
            label:
              linkedLibraryCertificate?.label ||
              application.name ||
              application.primaryDomain,
            activate: shouldActivate,
          },
        );
        config = await configManager.getConfig();
        deploymentChanged =
          deploymentChanged ||
          shouldActivate ||
          config.ssl.deployment_mode === "multi_sni";
      } catch (error: any) {
        console.error(
          `[ACME][cron] certificate library reconcile failed for ${application.primaryDomain}:`,
          error?.message || String(error),
        );
      }
    }

    const certificates = config.ssl.certificates || [];
    const activeCertificate =
      certificates.find(
        (certificate) => certificate.id === config.ssl.active_cert_id,
      ) || null;
    const hasAcmeCertificate = certificates.some(
      (certificate) => certificate.source === "acme",
    );
    const shouldSync =
      deploymentChanged ||
      (hasAcmeCertificate &&
        (config.ssl.deployment_mode === "multi_sni" ||
          activeCertificate?.source === "acme"));

    if (!shouldSync) return;
    await syncSSLDeploymentToGateway(config);
  } catch (error: any) {
    console.error(
      "[ACME][cron] SSL deployment reconcile failed:",
      error?.message || String(error),
    );
  }
};

export const registerAcmeRenewCron = (app: Elysia) => {
  const renewDays = Math.max(
    1,
    Math.min(90, parseIntSafe(process.env.ACME_RENEW_DAYS, 30)),
  );
  const thresholdMs = renewDays * 24 * 60 * 60 * 1000;
  const pattern = process.env.ACME_RENEW_CRON || "0 */6 * * *";
  const lockTtlSeconds = Math.max(
    60,
    Math.min(6 * 60 * 60, parseIntSafe(process.env.ACME_RENEW_LOCK_TTL, 3600)),
  );

  app.use(
    cron({
      name: "acme-auto-renew",
      pattern,
      async run() {
        const acquired = await configManager.setLockIfNotExists(
          "acme-renew",
          lockTtlSeconds,
        );
        if (!acquired) return;

        try {
          await acmeService.checkInstalled();
          if (acmeService.getState().status !== "installed") return;
          const activeLock = await configManager.getActiveAcmeRuntimeLock();
          if (activeLock.locked) return;

          const localeConfig = await configManager.getLocaleConfig();
          const applications = await configManager.listAcmeApplications();
          const renewableEntries: Array<{
            validToMs: number;
            application: (typeof applications)[number];
          }> = [];

          for (const application of applications) {
            if (!application.renewEnabled) continue;
            const issuedCertificate =
              await configManager.getUsableAcmeIssuedCertificate(
                application.id,
              );
            if (!issuedCertificate) continue;

            const validToMs = Date.parse(issuedCertificate.certInfo.validTo);
            if (!Number.isFinite(validToMs)) continue;
            if (
              !isExpiringSoon(issuedCertificate.certInfo.validTo, thresholdMs)
            )
              continue;

            renewableEntries.push({
              validToMs,
              application,
            });
          }

          renewableEntries.sort((a, b) => a.validToMs - b.validToMs);

          for (const entry of renewableEntries) {
            try {
              const started = await startAcmeApplicationJob({
                acme: acmeService,
                application: entry.application,
                trigger: "auto_renew",
                wait: true,
                locale: localeConfig.default_locale,
              });
              const latestJob = await configManager.getAcmeJob(started.job.id);
              if (latestJob?.status === "stopped") return;
            } catch (error: any) {
              const message = error?.message || String(error);
              if (
                message ===
                  activeAcmeTaskMessage(localeConfig.default_locale) ||
                message === tDefault("server.acmeJobRunner.activeTaskRunning")
              ) {
                return;
              }
              console.error(
                `[ACME][cron] renew failed for ${entry.application.primaryDomain}:`,
                message,
              );
            }
          }

          await reconcileAcmeSslDeployment();
        } catch (e: any) {
          console.error(
            "[ACME][cron] renew task error:",
            e?.message || String(e),
          );
        }
      },
    }),
  );

  return app;
};
