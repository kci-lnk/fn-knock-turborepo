import { homedir } from "node:os";
import { join, dirname } from "node:path";
import { chmod, cp, mkdir, readdir, readFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { ACME_EXECUTABLE_PATH, ACME_HOME_DIR, resolveBundledAcmeZipPath } from "../../lib/acme-paths";
import { collectStreamOutput, fileExists, waitForProcessExit } from "../../lib/runtime";

export type AcmeStatus = "uninstalled" | "installing" | "installed" | "error";

export interface AcmeState {
  status: AcmeStatus;
  progress: number;
  message: string;
  executablePath?: string;
}

export type VerifyMethod = "dns" | "http" | "https";
export type IssueCertParams = {
  domains: string[];
  method: VerifyMethod;
  dnsType?: string;
  envVars?: Record<string, string>;
  jobId?: string;
  onLog?: (line: string) => Promise<void> | void;
};

type AcmeCommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

export class AcmeService {
  private readonly currentDir = dirname(fileURLToPath(import.meta.url));
  private readonly legacyAcmeDir = join(homedir(), ".acme.sh");
  private readonly legacyAcmePath = join(this.legacyAcmeDir, "acme.sh");

  private state: AcmeState = {
    status: "uninstalled",
    progress: 0,
    message: "等待操作",
    executablePath: ACME_EXECUTABLE_PATH,
  };

  private get acmeDir() {
    return ACME_HOME_DIR;
  }

  private get acmePath() {
    return ACME_EXECUTABLE_PATH;
  }

  private isValidEmail(value: string | undefined | null): boolean {
    if (!value) return false;
    const v = value.trim();
    if (!v) return false;
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v);
  }

  private generateFallbackEmail(): string {
    const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
    return `acme-${suffix}@example.com`;
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

  private async runAcmeCommand(args: string[], extraEnv?: Record<string, string>): Promise<AcmeCommandResult> {
    const proc = spawn(this.acmePath, args, {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, ...(extraEnv || {}) },
    });
    const exitPromise = waitForProcessExit(proc);

    const [stdout, stderr, exitCode] = await Promise.all([
      collectStreamOutput(proc.stdout).catch(() => ""),
      collectStreamOutput(proc.stderr).catch(() => ""),
      exitPromise,
    ]);
    return { exitCode, stdout, stderr };
  }

  private async registerAccount(email?: string): Promise<string> {
    if (!email) {
      const existing = await this.getExistingAccountEmail();
      if (existing) return existing;
    }

    const accountEmail = this.resolveAccountEmail(email);
    const result = await this.runAcmeCommand([
      "--register-account",
      "-m",
      accountEmail,
      "--home",
      this.acmeDir,
      "--config-home",
      this.acmeDir,
      "--debug",
    ]);

    if (result.exitCode === 0) return accountEmail;

    const merged = `${result.stdout}\n${result.stderr}`.toLowerCase();
    const isAlreadyRegistered =
      (merged.includes("already") && merged.includes("account")) ||
      (merged.includes("already") && merged.includes("registered"));
    if (isAlreadyRegistered) return accountEmail;

    const brief = (result.stderr || result.stdout || "").trim().split("\n").slice(-3).join(" | ");
    throw new Error(`注册 ACME 账号失败（退出码: ${result.exitCode}）${brief ? `: ${brief}` : ""}`);
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
      throw new Error("未找到内置 acmesh.zip 资源");
    }

    this.state = { status: "installing", progress: 35, message: "正在解压内置 acme.sh 资源...", executablePath: this.acmePath };

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
        throw new Error(`解压失败，退出码: ${unzipCode}`);
      }

      const extractedRoot = await this.locateExtractedRoot(tmpDir);
      if (!extractedRoot) {
        throw new Error("解压成功但未找到 acme.sh");
      }

      this.state = { status: "installing", progress: 70, message: "正在写入数据目录...", executablePath: this.acmePath };
      await rm(this.acmeDir, { recursive: true, force: true });
      await mkdir(this.acmeDir, { recursive: true });
      await cp(extractedRoot, this.acmeDir, { recursive: true, force: true });

      if (!(await fileExists(this.acmePath))) {
        throw new Error("写入后未找到 acme.sh");
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
      this.state = {
        status: "error",
        progress: 0,
        message: `检查安装状态失败: ${e?.message || String(e)}`,
        executablePath: this.acmePath,
      };
      return false;
    }
    const exists = await fileExists(this.acmePath);
    if (exists) {
      this.state = {
        status: "installed",
        progress: 100,
        message: `acme.sh 已就绪 (${this.acmePath})`,
        executablePath: this.acmePath,
      };
      return true;
    }

    this.state = { status: "uninstalled", progress: 0, message: "acme.sh 未安装", executablePath: this.acmePath };
    return false;
  }

  getState(): AcmeState {
    return this.state;
  }

  async startInstall(email?: string): Promise<void> {
    if (this.state.status === "installing" || this.state.status === "installed") return;
    this.state = { status: "installing", progress: 10, message: "正在初始化内置 acme.sh...", executablePath: this.acmePath };

    try {
      const executablePath = await this.installFromBundledZip();
      this.state = { status: "installing", progress: 90, message: "正在注册 ACME 账号...", executablePath: this.acmePath };
      const accountEmail = await this.registerAccount(email);
      this.state = { status: "installed", progress: 100, message: `安装成功，账号邮箱: ${accountEmail}`, executablePath };
    } catch (error: any) {
      this.state = { status: "error", progress: 0, message: `安装失败: ${error.message}`, executablePath: this.acmePath };
    }
  }

  async uninstall(): Promise<void> {
    if (this.state.status === "installing") {
      throw new Error("acme.sh 正在安装中，无法删除");
    }

    try {
      await rm(this.acmeDir, { recursive: true, force: true });
      this.state = { status: "uninstalled", progress: 0, message: "acme.sh 已删除", executablePath: this.acmePath };
    } catch (error: any) {
      this.state = { status: "error", progress: 0, message: `删除失败: ${error?.message || String(error)}`, executablePath: this.acmePath };
      throw error;
    }
  }

  async issueCertificate({ domains, method, dnsType, envVars, onLog }: IssueCertParams) {
    if (this.state.status !== "installed") {
      throw new Error("请先安装 acme.sh");
    }

    if (!domains || domains.length === 0) {
      throw new Error("域名列表不能为空");
    }

    await this.registerAccount();

    const args: string[] = [this.acmePath, "--issue"];
    args.push("--home", this.acmeDir, "--config-home", this.acmeDir);
    if (method === "dns") {
      if (!dnsType) throw new Error("缺少 DNS 验证类型");
      args.push("--dns", dnsType);
    }

    args.push("--debug");
    
    // 循环添加所有的域名
    for (const d of domains) {
      args.push("-d", d);
    }
    
    const issueProc = spawn(args[0]!, args.slice(1), {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, ...(envVars || {}) }
    });
    const issueExitPromise = waitForProcessExit(issueProc);

    const p1 = this.processIssueStream(issueProc.stdout, onLog);
    const p2 = this.processIssueStream(issueProc.stderr, onLog);
    await Promise.all([p1, p2]);
    const exitCode = await issueExitPromise;
    if (exitCode !== 0) throw new Error(`证书签发失败，请检查 DNS API 密钥或网络状况。退出码: ${exitCode}`);
    
    return true;
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
