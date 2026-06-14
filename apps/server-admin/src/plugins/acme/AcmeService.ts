import { homedir } from "node:os";
import { join, dirname } from "node:path";
import { chmod, cp, mkdir, readdir, readFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { spawn, type ChildProcess } from "node:child_process";
import { ACME_EXECUTABLE_PATH, ACME_HOME_DIR, resolveBundledAcmeZipPath } from "../../lib/acme-paths";
import {
  DEFAULT_ACME_CERTIFICATE_AUTHORITY,
  type AcmeCertificateAuthority,
} from "../../lib/acme-certificate-authority";
import {
  normalizeAcmeDnsType,
  normalizeAcmeEnvVars,
} from "../../lib/acme-dns-providers";
import { collectStreamOutput, fileExists, sleep, waitForProcessExit } from "../../lib/runtime";
import { applyAcmeDnsProviderPatches } from "./patches/index";
import { tDefault } from "../../lib/i18n";

const acmeServiceT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.acmeService.${key}`, params);

type AcmeMessageParams = Record<
  string,
  string | number | boolean | null | undefined
>;
type AcmeMessageTranslator = (
  key: string,
  params?: AcmeMessageParams,
) => string;

export type AcmeStatus = "uninstalled" | "installing" | "installed" | "error";

export interface AcmeState {
  status: AcmeStatus;
  progress: number;
  message: string;
  messageKey?: string;
  messageParams?: AcmeMessageParams;
  executablePath?: string;
}

export type VerifyMethod = "dns" | "http" | "https";
export type IssueCertParams = {
  domains: string[];
  method: VerifyMethod;
  dnsType?: string;
  envVars?: Record<string, string>;
  certificateAuthority?: AcmeCertificateAuthority;
  force?: boolean;
  jobId?: string;
  onLog?: (line: string) => Promise<void> | void;
};

type AcmeCommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

export type StopAcmeProcessesResult = {
  matchedPids: number[];
  remainingPids: number[];
  errors: string[];
};

export class AcmeService {
  private readonly currentDir = dirname(fileURLToPath(import.meta.url));
  private readonly legacyAcmeDir = join(homedir(), ".acme.sh");
  private readonly legacyAcmePath = join(this.legacyAcmeDir, "acme.sh");
  private readonly activeAcmeProcesses = new Set<ChildProcess>();

  private state: AcmeState = this.createState({
    status: "uninstalled",
    progress: 0,
    messageKey: "waiting",
  });

  private get acmeDir() {
    return ACME_HOME_DIR;
  }

  private get acmePath() {
    return ACME_EXECUTABLE_PATH;
  }

  private createState({
    status,
    progress,
    messageKey,
    messageParams,
    executablePath = this.acmePath,
  }: {
    status: AcmeStatus;
    progress: number;
    messageKey: string;
    messageParams?: AcmeMessageParams;
    executablePath?: string;
  }): AcmeState {
    return {
      status,
      progress,
      message: acmeServiceT(messageKey, messageParams),
      messageKey,
      messageParams,
      executablePath,
    };
  }

  private trackAcmeProcess(proc: ChildProcess): ChildProcess {
    this.activeAcmeProcesses.add(proc);
    const cleanup = () => {
      this.activeAcmeProcesses.delete(proc);
    };
    proc.once("close", cleanup);
    proc.once("error", cleanup);
    return proc;
  }

  private spawnAcmeProcess(
    args: string[],
    extraEnv?: Record<string, string>,
  ): ChildProcess {
    return this.trackAcmeProcess(
      spawn(this.acmePath, args, {
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
        env: { ...process.env, ...(extraEnv || {}) },
      }),
    );
  }

  private async findAcmeProcessIds(): Promise<number[]> {
    const psProc = spawn("ps", ["-eo", "pid=,command="], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    const [stdout, , exitCode] = await Promise.all([
      collectStreamOutput(psProc.stdout).catch(() => ""),
      collectStreamOutput(psProc.stderr).catch(() => ""),
      waitForProcessExit(psProc).catch(() => -1),
    ]);
    if (exitCode !== 0) return [];

    const currentPid = process.pid;
    const ids: number[] = [];
    for (const line of stdout.split("\n")) {
      const matched = line.match(/^\s*(\d+)\s+(.+)$/);
      if (!matched) continue;
      const pid = Number(matched[1]);
      const command = matched[2] || "";
      if (!Number.isInteger(pid) || pid <= 0 || pid === currentPid) continue;
      if (!command.includes("acme.sh")) continue;
      ids.push(pid);
    }
    return Array.from(new Set(ids));
  }

  private collectTrackedAcmeProcessIds(): number[] {
    const ids: number[] = [];
    for (const proc of this.activeAcmeProcesses) {
      if (typeof proc.pid === "number" && proc.pid > 0) {
        ids.push(proc.pid);
      }
    }
    return ids;
  }

  private killPid(
    target: number,
    signal: NodeJS.Signals,
    errors: string[],
  ): void {
    try {
      process.kill(target, signal);
    } catch (error: any) {
      if (error?.code === "ESRCH") return;
      errors.push(
        acmeServiceT("sendSignalFailed", {
          detail: error?.message || String(error),
          signal,
          target,
        }),
      );
    }
  }

  async stopAllAcmeProcesses(): Promise<StopAcmeProcessesResult> {
    const trackedPids = this.collectTrackedAcmeProcessIds();
    const discoveredPids = await this.findAcmeProcessIds();
    const matchedPidSet = new Set([...trackedPids, ...discoveredPids]);
    const errors: string[] = [];

    for (const pid of trackedPids) {
      this.killPid(-pid, "SIGTERM", errors);
    }
    for (const pid of matchedPidSet) {
      this.killPid(pid, "SIGTERM", errors);
    }

    if (matchedPidSet.size > 0) {
      await sleep(1000);
    }

    const stillRunning = await this.findAcmeProcessIds();
    for (const pid of stillRunning) {
      matchedPidSet.add(pid);
    }
    for (const pid of trackedPids.filter((pid) =>
      stillRunning.includes(pid),
    )) {
      this.killPid(-pid, "SIGKILL", errors);
    }
    for (const pid of stillRunning) {
      this.killPid(pid, "SIGKILL", errors);
    }

    if (stillRunning.length > 0) {
      await sleep(300);
    }

    const finalRunning = await this.findAcmeProcessIds();
    return {
      matchedPids: Array.from(matchedPidSet),
      remainingPids: finalRunning,
      errors,
    };
  }

  private isValidEmail(value: string | undefined | null): boolean {
    if (!value) return false;
    const v = value.trim();
    if (!v) return false;
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v);
  }

  private generateFallbackEmail(): string {
    const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
    return `acme-${suffix}@fnknock.com`;
  }

  private resolveAccountEmail(email?: string): string {
    const fromArg = email?.trim();
    if (this.isValidEmail(fromArg)) return fromArg!;

    const fromEnv = process.env.ACME_ACCOUNT_EMAIL?.trim();
    if (this.isValidEmail(fromEnv)) return fromEnv!;

    return this.generateFallbackEmail();
  }

  private async getExistingAccountEmail(): Promise<string | null> {
    const accountConfCandidates = [
      join(this.acmeDir, "account.conf"),
      join(this.acmeDir, "ca", "acme.zerossl.com", "v2", "DV90", "account.conf"),
      join(this.acmeDir, "ca", "acme-v02.api.letsencrypt.org", "directory", "account.conf"),
    ];

    for (const confPath of accountConfCandidates) {
      const exists = await fileExists(confPath);
      if (!exists) continue;
      const content = await readFile(confPath, "utf-8").catch(() => "");
      const matched = content.match(/^ACCOUNT_EMAIL=(['"]?)([^\n'"]+)\1$/m);
      const existing = matched?.[2]?.trim();
      if (existing && this.isValidEmail(existing)) return existing;
    }

    return null;
  }

  private async readAccountConfValue(key: string): Promise<string | null> {
    const confPath = join(this.acmeDir, "account.conf");
    const exists = await fileExists(confPath);
    if (!exists) return null;
    const content = await readFile(confPath, "utf-8").catch(() => "");
    const matched = content.match(
      new RegExp(`^${key}=(['"]?)([^\\n'"]+)\\1$`, "m"),
    );
    return matched?.[2]?.trim() || null;
  }

  private async runAcmeCommand(args: string[], extraEnv?: Record<string, string>): Promise<AcmeCommandResult> {
    const proc = this.spawnAcmeProcess(args, extraEnv);
    const exitPromise = waitForProcessExit(proc);

    const [stdout, stderr, exitCode] = await Promise.all([
      collectStreamOutput(proc.stdout).catch(() => ""),
      collectStreamOutput(proc.stderr).catch(() => ""),
      exitPromise,
    ]);
    return { exitCode, stdout, stderr };
  }

  private buildSharedArgs(certificateAuthority?: AcmeCertificateAuthority): string[] {
    const args = ["--home", this.acmeDir, "--config-home", this.acmeDir];
    if (certificateAuthority) args.push("--server", certificateAuthority);
    return args;
  }

  private async setDefaultCertificateAuthority(
    certificateAuthority: AcmeCertificateAuthority,
  ): Promise<void> {
    const result = await this.runAcmeCommand([
      "--set-default-ca",
      "--server",
      certificateAuthority,
      ...this.buildSharedArgs(),
      "--debug",
    ]);

    if (result.exitCode === 0) return;

    const brief = (result.stderr || result.stdout || "")
      .trim()
      .split("\n")
      .slice(-3)
      .join(" | ");
    throw new Error(
      acmeServiceT("setDefaultCaFailed", {
        brief: brief ? `: ${brief}` : "",
        code: result.exitCode,
      }),
    );
  }

  async registerAccount(options?: {
    email?: string;
    certificateAuthority?: AcmeCertificateAuthority;
  }): Promise<string> {
    const accountEmail = this.resolveAccountEmail(
      options?.email || (await this.getExistingAccountEmail()) || undefined,
    );
    const result = await this.runAcmeCommand([
      "--register-account",
      "-m",
      accountEmail,
      ...this.buildSharedArgs(options?.certificateAuthority),
      "--debug",
    ]);

    if (result.exitCode === 0) return accountEmail;

    const merged = `${result.stdout}\n${result.stderr}`.toLowerCase();
    const isAlreadyRegistered =
      (merged.includes("already") && merged.includes("account")) ||
      (merged.includes("already") && merged.includes("registered"));
    if (isAlreadyRegistered) return accountEmail;

    const brief = (result.stderr || result.stdout || "").trim().split("\n").slice(-3).join(" | ");
    throw new Error(
      acmeServiceT("registerAccountFailed", {
        brief: brief ? `: ${brief}` : "",
        code: result.exitCode,
      }),
    );
  }

  private async migrateLegacyInstallIfNeeded(): Promise<void> {
    if (this.acmeDir === this.legacyAcmeDir) return;
    const exists = await fileExists(this.legacyAcmePath);
    if (!exists) return;

    await rm(this.acmeDir, { recursive: true, force: true });
    await mkdir(this.acmeDir, { recursive: true });
    await cp(this.legacyAcmeDir, this.acmeDir, { recursive: true, force: true });
    await chmod(this.acmePath, 0o755);
  }

  private async locateExtractedRoot(tmpDir: string): Promise<string | null> {
    const directCandidates = [join(tmpDir, "acmesh"), join(tmpDir, ".acme.sh"), tmpDir];
    for (const p of directCandidates) {
      const script = join(p, "acme.sh");
      if (await fileExists(script)) return p;
    }

    const entries = await readdir(tmpDir, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      if (entry.name === "__MACOSX") continue;
      const candidate = join(tmpDir, entry.name);
      const script = join(candidate, "acme.sh");
      if (await fileExists(script)) return candidate;
    }

    return null;
  }

  private async installFromBundledZip(): Promise<string> {
    const bundleZipPath = resolveBundledAcmeZipPath(this.currentDir);
    if (!bundleZipPath) {
      throw new Error(acmeServiceT("bundledZipMissing"));
    }

    this.state = this.createState({
      status: "installing",
      progress: 35,
      messageKey: "extractingBundled",
    });

    const tmpDir = join(this.acmeDir, "..", `.acme-extract-${Date.now()}-${Math.random().toString(16).slice(2)}`);
    await mkdir(tmpDir, { recursive: true });

    try {
      const unzipProc = spawn("unzip", ["-oq", bundleZipPath, "-d", tmpDir], {
        stdio: ["ignore", "pipe", "pipe"],
      });
      const unzipExitPromise = waitForProcessExit(unzipProc);

      const [, , unzipCode] = await Promise.all([
        collectStreamOutput(unzipProc.stdout).catch(() => ""),
        collectStreamOutput(unzipProc.stderr).catch(() => ""),
        unzipExitPromise,
      ]);
      if (unzipCode !== 0) {
        throw new Error(acmeServiceT("unzipFailed", { code: unzipCode }));
      }

      const extractedRoot = await this.locateExtractedRoot(tmpDir);
      if (!extractedRoot) {
        throw new Error(acmeServiceT("extractedAcmeMissing"));
      }

      this.state = this.createState({
        status: "installing",
        progress: 70,
        messageKey: "writingDataDir",
      });
      await rm(this.acmeDir, { recursive: true, force: true });
      await mkdir(this.acmeDir, { recursive: true });
      await cp(extractedRoot, this.acmeDir, { recursive: true, force: true });

      if (!(await fileExists(this.acmePath))) {
        throw new Error(acmeServiceT("writtenAcmeMissing"));
      }

      await chmod(this.acmePath, 0o755);
      return this.acmePath;
    } finally {
      await rm(tmpDir, { recursive: true, force: true });
    }
  }

  async checkInstalled(): Promise<boolean> {
    if (this.state.status === "installing") return false;

    try {
      await this.migrateLegacyInstallIfNeeded();
    } catch (e: any) {
      this.state = this.createState({
        status: "error",
        progress: 0,
        messageKey: "checkInstallFailed",
        messageParams: {
          detail: e?.message || String(e),
        },
      });
      return false;
    }
    const exists = await fileExists(this.acmePath);
    if (exists) {
      this.state = this.createState({
        status: "installed",
        progress: 100,
        messageKey: "ready",
      });
      return true;
    }

    this.state = this.createState({
      status: "uninstalled",
      progress: 0,
      messageKey: "notInstalled",
    });
    return false;
  }

  getState(): AcmeState {
    return this.state;
  }

  getLocalizedState(t: AcmeMessageTranslator): AcmeState {
    const state = this.getState();
    if (!state.messageKey) return state;
    return {
      ...state,
      message: t(`server.acmeService.${state.messageKey}`, state.messageParams),
    };
  }

  async getDefaultCertificateAuthority(): Promise<AcmeCertificateAuthority> {
    const configured = (
      await this.readAccountConfValue("DEFAULT_ACME_SERVER")
    )?.toLowerCase();
    if (configured?.includes("letsencrypt")) return "letsencrypt";
    if (configured?.includes("zerossl")) return "zerossl";
    return DEFAULT_ACME_CERTIFICATE_AUTHORITY;
  }

  async startInstall(
    email?: string,
    certificateAuthority?: AcmeCertificateAuthority,
  ): Promise<void> {
    if (this.state.status === "installing" || this.state.status === "installed")
      return;
    this.state = this.createState({
      status: "installing",
      progress: 10,
      messageKey: "initializingBundled",
    });

    try {
      const executablePath = await this.installFromBundledZip();
      this.state = this.createState({
        status: "installing",
        progress: 90,
        messageKey: "registeringAccount",
      });
      const accountEmail = await this.registerAccount({
        email,
        certificateAuthority,
      });
      this.state = this.createState({
        status: "installing",
        progress: 95,
        messageKey: "savingDefaultCa",
      });
      if (certificateAuthority) {
        await this.setDefaultCertificateAuthority(certificateAuthority);
      }
      this.state = this.createState({
        status: "installed",
        progress: 100,
        messageKey: "installSuccess",
        messageParams: { email: accountEmail },
        executablePath,
      });
    } catch (error: any) {
      this.state = this.createState({
        status: "error",
        progress: 0,
        messageKey: "installFailed",
        messageParams: { detail: error.message },
      });
    }
  }

  async switchCertificateAuthority(
    certificateAuthority: AcmeCertificateAuthority,
  ): Promise<string> {
    if (this.state.status !== "installed") {
      throw new Error(acmeServiceT("installFirst"));
    }

    const accountEmail = await this.registerAccount({ certificateAuthority });
    await this.setDefaultCertificateAuthority(certificateAuthority);
    return accountEmail;
  }

  async uninstall(): Promise<void> {
    if (this.state.status === "installing") {
      throw new Error(acmeServiceT("installingCannotDelete"));
    }

    try {
      await rm(this.acmeDir, { recursive: true, force: true });
      this.state = this.createState({
        status: "uninstalled",
        progress: 0,
        messageKey: "deleted",
      });
    } catch (error: any) {
      this.state = this.createState({
        status: "error",
        progress: 0,
        messageKey: "deleteFailed",
        messageParams: { detail: error?.message || String(error) },
      });
      throw error;
    }
  }

  async issueCertificate({
    domains,
    method,
    dnsType,
    envVars,
    onLog,
    certificateAuthority,
    force,
  }: IssueCertParams) {
    if (this.state.status !== "installed") {
      throw new Error(acmeServiceT("installFirst"));
    }

    if (!domains || domains.length === 0) {
      throw new Error(acmeServiceT("domainsRequired"));
    }

    const normalizedDnsType =
      method === "dns" ? normalizeAcmeDnsType(dnsType) : null;
    const recentLogLines: string[] = [];
    const appendRecentLog = async (line: string) => {
      const normalizedLine = line.trim();
      if (normalizedLine) {
        recentLogLines.push(normalizedLine);
        if (recentLogLines.length > 20) {
          recentLogLines.shift();
        }
      }
      if (onLog) {
        await onLog(line);
      }
    };

    if (method === "dns") {
      if (!normalizedDnsType) throw new Error(acmeServiceT("dnsTypeRequired"));
      await applyAcmeDnsProviderPatches({
        acmeHomeDir: this.acmeDir,
        dnsType: normalizedDnsType,
        onLog: appendRecentLog,
      });
    }

    await this.registerAccount({ certificateAuthority });

    const args: string[] = [this.acmePath, "--issue"];
    args.push("--home", this.acmeDir, "--config-home", this.acmeDir);
    if (certificateAuthority) {
      args.push("--server", certificateAuthority);
    }
    if (force !== false) {
      args.push("--force");
    }
    if (normalizedDnsType) {
      args.push("--dns", normalizedDnsType);
    }

    args.push("--debug");

    // 循环添加所有的域名
    for (const d of domains) {
      args.push("-d", d);
    }

    const issueProc = this.spawnAcmeProcess(
      args.slice(1),
      normalizeAcmeEnvVars(dnsType, envVars),
    );
    const issueExitPromise = waitForProcessExit(issueProc);

    const p1 = this.processIssueStream(issueProc.stdout, appendRecentLog);
    const p2 = this.processIssueStream(issueProc.stderr, appendRecentLog);
    await Promise.all([p1, p2]);
    const exitCode = await issueExitPromise;
    if (exitCode !== 0) {
      const brief = recentLogLines
        .filter((line) => !/^https?:\/\//i.test(line) && !/^v\d/i.test(line))
        .slice(-5)
        .join(" | ");
      throw new Error(
        acmeServiceT("issueFailed", {
          brief: brief ? `: ${brief}` : "",
          code: exitCode,
        }),
      );
    }

    return true;
  }

  async clearDomainWorkingState(domain: string): Promise<void> {
    const normalizedDomain = domain.trim().toLowerCase();
    if (!normalizedDomain) return;

    await Promise.all([
      rm(join(this.acmeDir, normalizedDomain), {
        recursive: true,
        force: true,
      }),
      rm(join(this.acmeDir, `${normalizedDomain}_ecc`), {
        recursive: true,
        force: true,
      }),
    ]);
  }

  private async processIssueStream(
    stream: NodeJS.ReadableStream | null | undefined,
    onLog?: (line: string) => Promise<void> | void,
  ) {
    if (!stream) return;
    let buf = "";
    for await (const chunk of stream) {
      const text = chunk.toString();
      buf += text;
      let idx: number;
      while ((idx = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, idx).replace(/\r$/, "");
        buf = buf.slice(idx + 1);
        if (onLog) await onLog(line);
      }
    }
    if (buf && onLog) {
      await onLog(buf);
    }
  }
}
