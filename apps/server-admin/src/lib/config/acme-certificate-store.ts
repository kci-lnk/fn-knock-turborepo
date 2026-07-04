import { spawn } from "node:child_process";
import { promises as fs } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type Redis from "ioredis";
import { dataPath } from "../AppDirManager";
import { ACME_EXECUTABLE_PATH, ACME_HOME_DIR } from "../acme-paths";
import {
  collectStreamOutput,
  fileExists,
  waitForProcessExit,
} from "../runtime";
import type { SSLCertInfo } from "./types";

export interface AcmeCertificatePair {
  cert: string;
  key: string;
}

export class AcmeCertificateStore {
  private acmeCertKey = "fn_knock:acme:cert:";

  constructor(
    private readonly redis: Redis,
    private readonly parseCertInfo: (certPem: string) => SSLCertInfo | null,
  ) {}

  async save(
    domain: string,
    cert: string,
    keyPem: string,
  ): Promise<void> {
    await this.redis.set(
      `${this.acmeCertKey}${domain}`,
      JSON.stringify({ cert, key: keyPem }),
    );
  }

  async get(domain: string): Promise<AcmeCertificatePair | null> {
    const raw = await this.redis.get(`${this.acmeCertKey}${domain}`);
    if (!raw) return null;
    try {
      const obj = JSON.parse(raw);
      if (
        typeof obj?.cert === "string" &&
        typeof obj?.key === "string" &&
        obj.cert.trim() &&
        obj.key.trim()
      ) {
        return obj;
      }
      return null;
    } catch {
      return null;
    }
  }

  async delete(domain: string): Promise<void> {
    await this.redis.del(`${this.acmeCertKey}${domain}`);
  }

  async getInfo(domain: string): Promise<SSLCertInfo | null> {
    const pair = await this.get(domain);
    if (!pair) return null;
    return this.parseCertInfo(pair.cert);
  }

  async saveFromFS(
    domain: string,
    opts?: {
      forceInstall?: boolean;
      onLog?: (line: string) => Promise<void> | void;
    },
  ): Promise<boolean> {
    const domainDir = join(dataPath, "ssl", domain);
    const installedKeyPath = join(domainDir, `${domain}.key`);
    const installedFullchainPath = join(domainDir, "fullchain.cer");
    const normalizedDomain = domain.trim().toLowerCase();
    const acmeDirCandidates = [
      {
        dir: join(ACME_HOME_DIR, `${normalizedDomain}_ecc`),
        useEcc: true,
      },
      {
        dir: join(ACME_HOME_DIR, normalizedDomain),
        useEcc: false,
      },
      {
        dir: join(homedir(), ".acme.sh", `${normalizedDomain}_ecc`),
        useEcc: true,
      },
      {
        dir: join(homedir(), ".acme.sh", normalizedDomain),
        useEcc: false,
      },
    ];

    const appendLog = async (line: string) => {
      const normalized = line.trim();
      if (!normalized || !opts?.onLog) return;
      await opts.onLog(normalized);
    };

    const summarizeCommandOutput = (stdout: string, stderr: string): string => {
      const merged = `${stdout}\n${stderr}`
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)
        .slice(-5)
        .join(" | ");
      return merged || "no output";
    };

    try {
      const hasKey = await fileExists(installedKeyPath);
      const hasFullchain = await fileExists(installedFullchainPath);
      const shouldInstall = !!opts?.forceInstall || !hasKey || !hasFullchain;

      if (shouldInstall) {
        await fs.mkdir(domainDir, { recursive: true });
        const exists = await fileExists(ACME_EXECUTABLE_PATH);
        if (!exists) return false;

        const existingCandidates: typeof acmeDirCandidates = [];
        for (const candidate of acmeDirCandidates) {
          if (await fileExists(candidate.dir)) {
            existingCandidates.push(candidate);
          }
        }

        const installVariants =
          existingCandidates.length > 0
            ? [
                ...new Set(
                  existingCandidates.map((candidate) => candidate.useEcc),
                ),
              ]
            : [true, false];

        let installSucceeded = false;
        for (const useEcc of installVariants) {
          const installArgs = [
            "--home",
            ACME_HOME_DIR,
            "--config-home",
            ACME_HOME_DIR,
            "--install-cert",
            "-d",
            domain,
            "--key-file",
            installedKeyPath,
            "--fullchain-file",
            installedFullchainPath,
          ];
          if (useEcc) {
            installArgs.push("--ecc");
          }

          const installProc = spawn(ACME_EXECUTABLE_PATH, installArgs, {
            stdio: ["ignore", "pipe", "pipe"],
          });
          const installExitPromise = waitForProcessExit(installProc);

          const [stdout, stderr, exitCode] = await Promise.all([
            collectStreamOutput(installProc.stdout).catch(() => ""),
            collectStreamOutput(installProc.stderr).catch(() => ""),
            installExitPromise,
          ]);
          if (exitCode === 0) {
            installSucceeded = true;
            break;
          }
          await appendLog(
            `[acme][install-cert] ${useEcc ? "ECC" : "RSA"} install failed (exit ${exitCode}): ${summarizeCommandOutput(stdout, stderr)}`,
          );
        }

        if (!installSucceeded) return false;
      }

      const cert = await fs.readFile(installedFullchainPath, "utf-8");
      const key = await fs.readFile(installedKeyPath, "utf-8");
      if (!cert.trim() || !key.trim()) return false;
      if (!this.parseCertInfo(cert)) return false;
      await this.save(domain, cert, key);
      return true;
    } catch (error: any) {
      await appendLog(
        `[acme][install-cert] failed to install certificate files: ${error?.message || String(error)}`,
      );
      try {
        for (const candidate of acmeDirCandidates) {
          const certPathA = join(candidate.dir, "fullchain.cer");
          const certPathB = join(candidate.dir, `${normalizedDomain}.cer`);
          const keyPath = join(candidate.dir, `${normalizedDomain}.key`);
          try {
            const cert = await fs
              .readFile(certPathA, "utf-8")
              .catch(async () => await fs.readFile(certPathB, "utf-8"));
            const key = await fs.readFile(keyPath, "utf-8");
            if (!cert.trim() || !key.trim()) continue;
            if (!this.parseCertInfo(cert)) continue;
            await this.save(domain, cert, key);
            return true;
          } catch {
            // try next fallback directory
          }
        }
        await appendLog(
          "[acme][install-cert] fallback certificate read failed for all known acme.sh directories",
        );
        return false;
      } catch {
        return false;
      }
    }
  }
}
