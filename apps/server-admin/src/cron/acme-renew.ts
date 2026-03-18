import type { Elysia } from "elysia";
import { cron } from "@elysiajs/cron";
import { randomUUID } from "node:crypto";
import { acmeService } from "../plugins/acme";
import { configManager } from "../lib/redis";
import { syncSSLDeploymentToGateway } from "../lib/ssl-gateway";

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
          const settings = await configManager.getAcmeSettings();
          if (!settings?.domains?.length) return;

          await acmeService.checkInstalled();
          if (acmeService.getState().status !== "installed") return;

          const primaryDomain = settings.domains[0]!;
          let storedPair = await configManager.getAcmeCert(primaryDomain);
          if (!storedPair) {
            const loaded =
              await configManager.saveAcmeCertFromFS(primaryDomain);
            if (!loaded) return;
            storedPair = await configManager.getAcmeCert(primaryDomain);
            if (!storedPair) return;
          }

          const activeCertificate =
            await configManager.getActiveSSLCertificate();
          if (!activeCertificate) return;
          if (
            activeCertificate.source !== "acme" ||
            activeCertificate.primary_domain !== primaryDomain
          ) {
            return;
          }

          const sslStatus = await configManager.getSSLStatus();
          const shouldRenew = isExpiringSoon(
            sslStatus.certInfo?.validTo,
            thresholdMs,
          );
          if (!shouldRenew) return;

          const jobId = randomUUID();
          await configManager.createAcmeJob({
            id: jobId,
            domains: settings.domains,
            method: "dns",
            provider: settings.dnsType,
            createdAt: new Date().toISOString(),
            status: "running",
            progress: 5,
            message: "running",
          });
          await configManager.clearAcmeLogs(jobId);
          await configManager.appendAcmeLog(
            jobId,
            `[cron] renew start: ${primaryDomain}`,
          );

          const logRing: string[] = [];
          const pushLog = (line: string) => {
            const v = String(line ?? "");
            if (!v) return;
            logRing.push(v);
            if (logRing.length > 40) logRing.shift();
          };

          try {
            await acmeService.issueCertificate({
              domains: settings.domains,
              method: "dns",
              dnsType: settings.dnsType,
              envVars: settings.credentials,
              onLog: async (line: string) => {
                pushLog(line);
                await configManager.appendAcmeLog(jobId, line);
              },
            });

            await configManager.updateAcmeJob(jobId, {
              progress: 80,
              message: "saving",
            });
            const saved = await configManager.saveAcmeCertFromFS(
              primaryDomain,
              { forceInstall: true },
            );
            if (!saved) throw new Error("证书签发成功，但读取证书文件失败");

            await configManager.saveAcmeCertificateToLibrary(primaryDomain, {
              id: activeCertificate.id,
              label: activeCertificate.label,
              activate: true,
            });
            await syncSSLDeploymentToGateway();

            await configManager.appendAcmeLog(jobId, "[cron] renew succeeded");
            await configManager.updateAcmeJob(jobId, {
              status: "succeeded",
              progress: 100,
              message: "succeeded",
            });
          } catch (e: any) {
            const msg = e?.message || String(e);
            const tail = logRing.slice(-10).join("\n");
            if (tail)
              await configManager.appendAcmeLog(
                jobId,
                `[cron] last logs:\n${tail}`,
              );
            await configManager.appendAcmeLog(
              jobId,
              `[cron] renew failed: ${msg}`,
            );
            await configManager.updateAcmeJob(jobId, {
              status: "failed",
              progress: 100,
              message: msg,
            });
          }
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
