import { spawn } from "node:child_process";
import { constants as fsConstants } from "node:fs";
import type { FileHandle } from "node:fs/promises";
import { mkdir, open, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { v4 as uuidv4 } from "uuid";
import { dataPath } from "./AppDirManager";
import { homedir } from "node:os";
import { configManager } from "./redis";
import { collectStreamOutput, sleep, waitForProcessExit } from "./runtime";
import { terminalStore } from "./terminal-store";
import {
  DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS,
  DEFAULT_TERMINAL_POLL_INTERVAL_MS,
  DEFAULT_TERMINAL_POLL_TIMEOUT_MS,
  TerminalAttachmentRecord,
  TerminalFeatureConfig,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTmuxDetectionSource,
  TerminalTmuxInstallState,
  normalizeTerminalAttachmentRecord,
  normalizeTerminalSessionRecord,
} from "./terminal-shared";

const DEFAULT_CWD = homedir();

const DEFAULT_SHELL = process.env.SHELL || "/bin/sh";
const TMUX_TARGET_PANE_SUFFIX = ":0.0";
const TERMINAL_STREAM_DIR_NAME = "terminal-streams";
const TERMINAL_STREAM_CHUNK_MAX_BYTES = 256 * 1024;
const INPUT_SESSION_TOUCH_THROTTLE_MS = 5_000;
const INPUT_PIPE_OPEN_FLAGS =
  fsConstants.O_WRONLY | (fsConstants.O_NONBLOCK ?? 0);
const DEFAULT_SESSION_TITLE_PREFIX = "会话-";
const TMUX_ABSOLUTE_FALLBACK_PATH = "/usr/bin/tmux";
const DEBIAN_APT_GET_PATH = "/usr/bin/apt-get";
const TERMINAL_RELAY_NODE_SCRIPT = [
  "const fs=require('node:fs');",
  "const [logPath,inputPath]=process.argv.slice(-2);",
  "const log=fs.createWriteStream(logPath,{flags:'a'});",
  "const inputFd=fs.openSync(inputPath,'r+');",
  "const input=fs.createReadStream(null,{fd:inputFd,autoClose:true});",
  "log.on('error',()=>process.exit(1));",
  "input.on('error',()=>process.exit(1));",
  "process.stdout.on('error',()=>process.exit(0));",
  "process.stdin.pipe(log);",
  "input.pipe(process.stdout);",
].join("");

type ExecResult = {
  code: number;
  stdout: string;
  stderr: string;
};

type CreateSessionInput = {
  title?: string;
  shell?: string;
  cwd?: string;
  cols?: number;
  rows?: number;
};

type TmuxExecutableInfo = {
  path: string;
  detectionSource: TerminalTmuxDetectionSource;
  version: string;
};

const parseTmuxNumber = (value: string, fallback: number): number => {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
};

const parseOutputCursor = (
  value: number | string | null | undefined,
  fallback = 0,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }
  return parsed;
};

const shellQuote = (value: string): string =>
  `'${value.replace(/'/g, `'\"'\"'`)}'`;

class TerminalManager {
  private tmuxExecutableInfoPromise: Promise<TmuxExecutableInfo | null> | null =
    null;
  private tmuxInstallPromise: Promise<void> | null = null;
  private tmuxInstallState: TerminalTmuxInstallState = {
    status: "uninstalled",
    progress: 0,
    message: "未检测到 tmux，请先安装 Debian tmux 环境",
    executablePath: "",
    detectionSource: null,
    version: "",
  };
  private readonly inputPipeWriterHandles = new Map<
    string,
    Promise<FileHandle>
  >();
  private readonly sessionActivityTouchDeadlines = new Map<string, number>();
  private readonly streamDirectory = join(dataPath, TERMINAL_STREAM_DIR_NAME);

  private async runProcess(
    command: string,
    args: string[],
    options: { cwd?: string; trimOutput?: boolean } = {},
  ): Promise<ExecResult> {
    const proc = spawn(command, args, {
      cwd: options.cwd,
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });

    const [stdout, stderr, code] = await Promise.all([
      collectStreamOutput(proc.stdout),
      collectStreamOutput(proc.stderr),
      waitForProcessExit(proc),
    ]);

    if (options.trimOutput === false) {
      return { code, stdout, stderr };
    }

    return { code, stdout: stdout.trimEnd(), stderr: stderr.trimEnd() };
  }

  private async runTmux(args: string[]): Promise<ExecResult> {
    const tmuxInfo = await this.detectTmuxExecutable();
    return this.runProcess(tmuxInfo?.path || "tmux", args);
  }

  private resetTmuxProbeCache(): void {
    this.tmuxExecutableInfoPromise = null;
  }

  private summarizeProcessOutput(result: ExecResult): string {
    const detail = `${result.stderr}\n${result.stdout}`.trim();
    if (!detail) return "";
    const lines = detail
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    return lines.slice(-8).join(" | ").slice(0, 500);
  }

  private async ensureProcessSucceeded(
    command: string,
    args: string[],
    failureMessage: string,
  ): Promise<ExecResult> {
    const result = await this.runProcess(command, args, { trimOutput: false });
    if (result.code === 0) {
      return {
        ...result,
        stdout: result.stdout.trimEnd(),
        stderr: result.stderr.trimEnd(),
      };
    }

    const detail = this.summarizeProcessOutput(result);
    throw new Error(detail ? `${failureMessage}: ${detail}` : failureMessage);
  }

  private async detectTmuxExecutable(): Promise<TmuxExecutableInfo | null> {
    if (!this.tmuxExecutableInfoPromise) {
      this.tmuxExecutableInfoPromise = (async () => {
        const candidates: Array<{
          path: string;
          detectionSource: TerminalTmuxDetectionSource;
        }> = [
          { path: "tmux", detectionSource: "env-path" },
          {
            path: TMUX_ABSOLUTE_FALLBACK_PATH,
            detectionSource: "absolute-path",
          },
        ];

        for (const candidate of candidates) {
          try {
            const result = await this.runProcess(candidate.path, ["-V"]);
            if (result.code !== 0) {
              continue;
            }

            return {
              path: candidate.path,
              detectionSource: candidate.detectionSource,
              version: result.stdout || "tmux",
            };
          } catch {
            continue;
          }
        }

        return null;
      })();
    }

    const tmuxInfo = await this.tmuxExecutableInfoPromise;
    if (!tmuxInfo) {
      this.tmuxExecutableInfoPromise = null;
    }
    return tmuxInfo;
  }

  private async getTmuxInstallState(): Promise<TerminalTmuxInstallState> {
    if (this.tmuxInstallState.status === "installing") {
      return this.tmuxInstallState;
    }

    const tmuxInfo = await this.detectTmuxExecutable();
    if (tmuxInfo) {
      this.tmuxInstallState = {
        status: "installed",
        progress: 100,
        message: `tmux 已就绪：${tmuxInfo.version}`,
        executablePath: tmuxInfo.path,
        detectionSource: tmuxInfo.detectionSource,
        version: tmuxInfo.version,
      };
      return this.tmuxInstallState;
    }

    if (this.tmuxInstallState.status === "error") {
      return this.tmuxInstallState;
    }

    this.tmuxInstallState = {
      status: "uninstalled",
      progress: 0,
      message: "未检测到 tmux，请先安装 Debian tmux 环境",
      executablePath: "",
      detectionSource: null,
      version: "",
    };
    return this.tmuxInstallState;
  }

  private async installTmuxInBackground(): Promise<void> {
    try {
      this.tmuxInstallState = {
        status: "installing",
        progress: 15,
        message: "正在刷新 Debian 软件源...",
        executablePath: "",
        detectionSource: null,
        version: "",
      };
      await this.ensureProcessSucceeded(
        DEBIAN_APT_GET_PATH,
        ["update"],
        "apt-get update 执行失败",
      );

      this.tmuxInstallState = {
        status: "installing",
        progress: 60,
        message: "正在安装 tmux...",
        executablePath: "",
        detectionSource: null,
        version: "",
      };
      await this.ensureProcessSucceeded(
        DEBIAN_APT_GET_PATH,
        ["install", "-y", "tmux"],
        "apt-get install tmux 执行失败",
      );

      this.tmuxInstallState = {
        status: "installing",
        progress: 90,
        message: "正在验证 tmux 安装结果...",
        executablePath: "",
        detectionSource: null,
        version: "",
      };
      this.resetTmuxProbeCache();

      const tmuxInfo = await this.detectTmuxExecutable();
      if (!tmuxInfo) {
        throw new Error("安装完成后仍未检测到 tmux");
      }

      this.tmuxInstallState = {
        status: "installed",
        progress: 100,
        message: `tmux 安装完成：${tmuxInfo.version}`,
        executablePath: tmuxInfo.path,
        detectionSource: tmuxInfo.detectionSource,
        version: tmuxInfo.version,
      };
    } catch (error) {
      this.resetTmuxProbeCache();
      this.tmuxInstallState = {
        status: "error",
        progress: 0,
        message:
          error instanceof Error && error.message.trim()
            ? error.message.trim()
            : "tmux 安装失败",
        executablePath: "",
        detectionSource: null,
        version: "",
      };
    }
  }

  async startTmuxInstall(): Promise<TerminalTmuxInstallState> {
    const current = await this.getTmuxInstallState();
    if (current.status === "installed" || current.status === "installing") {
      return current;
    }

    if (!this.tmuxInstallPromise) {
      this.tmuxInstallPromise = this.installTmuxInBackground().finally(() => {
        this.tmuxInstallPromise = null;
      });
    }

    return this.tmuxInstallState;
  }

  private async ensureDirectoryExists(path: string): Promise<void> {
    const info = await stat(path).catch(() => null);
    if (!info?.isDirectory()) {
      throw new Error(`工作目录不存在或不可访问: ${path}`);
    }
  }

  private async ensureStreamDirectory(): Promise<void> {
    await mkdir(this.streamDirectory, { recursive: true });
  }

  private buildSessionName(id: string): string {
    return `fnk_${id.replace(/-/g, "").slice(0, 16)}`;
  }

  private buildOutputLogPath(id: string): string {
    return join(this.streamDirectory, `${id}.log`);
  }

  private buildInputPipePath(id: string): string {
    return join(this.streamDirectory, `${id}.in`);
  }

  private paneTarget(session: TerminalSessionRecord): string {
    return `${session.backend_session_name}${TMUX_TARGET_PANE_SUFFIX}`;
  }

  private sanitizeTitle(rawTitle: string | undefined): string {
    const trimmed = (rawTitle || "").trim();
    return trimmed;
  }

  private buildDefaultSessionTitle(
    existingSessions: TerminalSessionRecord[],
  ): string {
    const usedIndexes = new Set<number>();
    for (const session of existingSessions) {
      const match = session.title.trim().match(/^会话-(\d+)$/);
      if (!match) continue;
      const parsed = Number.parseInt(match[1]!, 10);
      if (Number.isFinite(parsed) && parsed > 0) {
        usedIndexes.add(parsed);
      }
    }

    let nextIndex = 1;
    while (usedIndexes.has(nextIndex)) {
      nextIndex += 1;
    }

    return `${DEFAULT_SESSION_TITLE_PREFIX}${nextIndex}`;
  }

  private formatIoError(prefix: string, error: unknown): string {
    const detail = error instanceof Error ? error.message.trim() : "";
    return detail ? `${prefix}: ${detail}` : prefix;
  }

  private buildRelayCommand(
    outputLogPath: string,
    inputPipePath: string,
  ): string {
    return [
      shellQuote(process.execPath),
      "-e",
      shellQuote(TERMINAL_RELAY_NODE_SCRIPT),
      shellQuote(outputLogPath),
      shellQuote(inputPipePath),
    ].join(" ");
  }

  private async getFeatureConfig(): Promise<TerminalFeatureConfig> {
    return configManager.getTerminalFeatureConfig();
  }

  async getRuntimeStatus(): Promise<TerminalRuntimeStatus> {
    const [config, tmuxInstallState] = await Promise.all([
      this.getFeatureConfig(),
      this.getTmuxInstallState(),
    ]);
    const tmuxAvailable = tmuxInstallState.status === "installed";
    const runningAsRoot = (process.getuid?.() ?? -1) === 0;

    let blockedReason = "";
    if (!config.enabled) {
      blockedReason = "网页终端功能尚未启用";
    } else if (tmuxInstallState.status === "installing") {
      blockedReason = "tmux 安装中，请等待安装完成";
    } else if (!tmuxAvailable) {
      blockedReason =
        tmuxInstallState.status === "error"
          ? `tmux 状态异常：${tmuxInstallState.message}`
          : "未检测到 tmux，无法创建可恢复终端会话";
    } else if (runningAsRoot && !config.dangerously_run_as_current_user) {
      blockedReason =
        "当前进程以 root 运行，需在设置中显式开启高危运行开关后才能创建终端";
    }

    return {
      enabled: config.enabled,
      tmuxAvailable,
      tmuxExecutablePath: tmuxInstallState.executablePath,
      tmuxDetectionSource: tmuxInstallState.detectionSource,
      tmuxVersion: tmuxInstallState.version,
      tmuxInstallState,
      httpPollingAvailable: true,
      runningAsRoot,
      blockedReason,
    };
  }

  async isTmuxAvailable(): Promise<boolean> {
    const tmuxInfo = await this.detectTmuxExecutable();
    return Boolean(tmuxInfo);
  }

  async getTmuxVersion(): Promise<string | null> {
    const tmuxInfo = await this.detectTmuxExecutable();
    return tmuxInfo?.version || null;
  }

  private async assertCreateAllowed(): Promise<void> {
    const status = await this.getRuntimeStatus();
    if (status.blockedReason) {
      throw new Error(status.blockedReason);
    }
  }

  private async resolveShell(shell?: string): Promise<string> {
    const config = await this.getFeatureConfig();
    return (
      (shell || config.default_shell || DEFAULT_SHELL).trim() || DEFAULT_SHELL
    );
  }

  private async resolveCwd(cwd?: string): Promise<string> {
  const config = await this.getFeatureConfig();
  const configuredCwd = (config.default_cwd || "").trim();
  const nextCwd = (cwd || configuredCwd).trim();

  const resolvedCwd =
    !nextCwd || nextCwd === "~"
      ? DEFAULT_CWD
      : nextCwd.startsWith("~/")
        ? join(DEFAULT_CWD, nextCwd.slice(2))
        : nextCwd;

  await this.ensureDirectoryExists(resolvedCwd);
  return resolvedCwd;
}

  private async refreshSessionExpiry(
    session: TerminalSessionRecord,
  ): Promise<TerminalSessionRecord> {
    const config = await this.getFeatureConfig();
    const now = Date.now();
    return terminalStore.saveSession(
      normalizeTerminalSessionRecord({
        ...session,
        updated_at: new Date(now).toISOString(),
        expires_at: new Date(
          now + config.idle_timeout_seconds * 1000,
        ).toISOString(),
      }),
    );
  }

  private async touchSessionActivity(
    session: TerminalSessionRecord,
    options: { force?: boolean } = {},
  ): Promise<TerminalSessionRecord> {
    const now = Date.now();
    const normalized = normalizeTerminalSessionRecord({
      ...session,
      updated_at: new Date(now).toISOString(),
    });
    const nextAllowedAt =
      this.sessionActivityTouchDeadlines.get(session.id) || 0;

    if (!options.force && now < nextAllowedAt) {
      return normalized;
    }

    const saved = await this.refreshSessionExpiry(normalized);
    this.sessionActivityTouchDeadlines.set(
      session.id,
      now + INPUT_SESSION_TOUCH_THROTTLE_MS,
    );
    return saved;
  }

  private async closeInputPipeWriter(sessionId: string): Promise<void> {
    const writerPromise = this.inputPipeWriterHandles.get(sessionId);
    if (!writerPromise) return;
    this.inputPipeWriterHandles.delete(sessionId);
    const writer = await writerPromise.catch(() => null);
    await writer?.close().catch(() => undefined);
  }

  private async cleanupSessionArtifacts(
    session: Pick<
      TerminalSessionRecord,
      "id" | "input_pipe_path" | "output_log_path"
    >,
  ): Promise<void> {
    this.sessionActivityTouchDeadlines.delete(session.id);
    await this.closeInputPipeWriter(session.id);
    const inputPipePath = session.input_pipe_path?.trim();
    if (inputPipePath) {
      await rm(inputPipePath, { force: true }).catch(() => undefined);
    }
    const outputLogPath = session.output_log_path?.trim();
    if (outputLogPath) {
      await rm(outputLogPath, { force: true }).catch(() => undefined);
    }
  }

  async cleanupExpiredSessions(): Promise<void> {
    const sessions = await terminalStore.listSessions();
    const now = Date.now();

    for (const session of sessions) {
      const expiresAt = Date.parse(session.expires_at || "");
      if (Number.isFinite(expiresAt) && expiresAt <= now) {
        await this.killSession(session.id).catch((error) => {
          console.error("[terminal] failed to cleanup expired session:", error);
        });
        continue;
      }

      const exists = await this.tmuxSessionExists(session.backend_session_name);
      if (!exists) {
        await this.cleanupSessionArtifacts(session).catch(() => undefined);
        await terminalStore.deleteSession(session.id);
        continue;
      }

      if (session.status === "attached") {
        const liveAttachmentIds =
          await terminalStore.listAttachmentIdsForSession(session.id);
        if (liveAttachmentIds.length === 0) {
          await this.markSessionDetached(session.id).catch((error) => {
            console.error(
              "[terminal] failed to mark stale attachment session:",
              error,
            );
          });
        }
      }
    }
  }

  private async tmuxSessionExists(sessionName: string): Promise<boolean> {
    try {
      const result = await this.runTmux(["has-session", "-t", sessionName]);
      return result.code === 0;
    } catch {
      return false;
    }
  }

  async listSessions(): Promise<TerminalSessionRecord[]> {
    await this.cleanupExpiredSessions();
    return terminalStore.listSessions();
  }

  async getSession(id: string): Promise<TerminalSessionRecord | null> {
    const session = await terminalStore.getSession(id);
    if (!session) return null;
    const exists = await this.tmuxSessionExists(session.backend_session_name);
    if (!exists) {
      await this.cleanupSessionArtifacts(session).catch(() => undefined);
      await terminalStore.deleteSession(id);
      return null;
    }
    return session;
  }

  private async readPaneRuntimeMetadata(
    session: TerminalSessionRecord,
  ): Promise<{
    paneTtyPath: string;
    cols: number;
    rows: number;
  }> {
    const paneResult = await this.runTmux([
      "display-message",
      "-p",
      "-t",
      this.paneTarget(session),
      "#{pane_tty}\t#{pane_width}\t#{pane_height}",
    ]);
    if (paneResult.code !== 0) {
      throw new Error(paneResult.stderr || "无法读取终端 pane 元数据");
    }

    const [paneTtyPathRaw = "", colsRaw = "", rowsRaw = ""] = (
      paneResult.stdout || ""
    ).split("\t");
    const paneTtyPath = paneTtyPathRaw.trim();
    if (!paneTtyPath) {
      throw new Error("无法解析终端 pane tty");
    }

    return {
      paneTtyPath,
      cols: parseTmuxNumber(colsRaw, session.cols),
      rows: parseTmuxNumber(rowsRaw, session.rows),
    };
  }

  private async isRelayPipeActive(
    session: TerminalSessionRecord,
  ): Promise<boolean> {
    const result = await this.runTmux([
      "display-message",
      "-p",
      "-t",
      this.paneTarget(session),
      "#{?pane_pipe,1,0}",
    ]);
    return result.code === 0 && result.stdout.trim() === "1";
  }

  private async ensureOutputLogPath(
    session: TerminalSessionRecord,
  ): Promise<string> {
    await this.ensureStreamDirectory();
    const outputLogPath =
      session.output_log_path.trim() || this.buildOutputLogPath(session.id);
    await writeFile(outputLogPath, "", { flag: "a" });
    return outputLogPath;
  }

  private async ensureInputPipePath(
    session: TerminalSessionRecord,
  ): Promise<string> {
    await this.ensureStreamDirectory();
    const inputPipePath =
      session.input_pipe_path.trim() || this.buildInputPipePath(session.id);
    const inputPipeInfo = await stat(inputPipePath).catch(() => null);

    if (inputPipeInfo) {
      if (inputPipeInfo.isFIFO()) {
        return inputPipePath;
      }
      await rm(inputPipePath, { force: true }).catch(() => undefined);
    }

    const result = await this.runProcess("mkfifo", [inputPipePath]);
    if (result.code !== 0) {
      throw new Error(result.stderr || "无法创建终端输入管道");
    }
    return inputPipePath;
  }

  private async configureRelayPipe(
    session: TerminalSessionRecord,
    outputLogPath: string,
    inputPipePath: string,
  ): Promise<void> {
    const result = await this.runTmux([
      "pipe-pane",
      "-I",
      "-O",
      "-t",
      this.paneTarget(session),
      this.buildRelayCommand(outputLogPath, inputPipePath),
    ]);
    if (result.code !== 0) {
      throw new Error(result.stderr || "无法建立终端 IO 中继");
    }
  }

  private async configureSessionRuntime(
    session: TerminalSessionRecord,
  ): Promise<TerminalSessionRecord> {
    await this.closeInputPipeWriter(session.id);
    const outputLogPath = await this.ensureOutputLogPath(session);
    const inputPipePath = await this.ensureInputPipePath(session);
    const paneInfo = await this.readPaneRuntimeMetadata(session);
    await this.configureRelayPipe(session, outputLogPath, inputPipePath);

    return terminalStore.saveSession(
      normalizeTerminalSessionRecord({
        ...session,
        cols: paneInfo.cols,
        rows: paneInfo.rows,
        pane_tty_path: paneInfo.paneTtyPath,
        input_pipe_path: inputPipePath,
        output_log_path: outputLogPath,
        updated_at: new Date().toISOString(),
      }),
    );
  }

  private async ensureSessionRuntime(
    session: TerminalSessionRecord,
  ): Promise<TerminalSessionRecord> {
    const outputLogPath =
      session.output_log_path.trim() || this.buildOutputLogPath(session.id);
    const inputPipePath =
      session.input_pipe_path.trim() || this.buildInputPipePath(session.id);
    const hasPaneTtyPath = session.pane_tty_path.trim().length > 0;
    const outputLogExists = await stat(outputLogPath)
      .then((info) => info.isFile())
      .catch(() => false);
    const inputPipeExists = await stat(inputPipePath)
      .then((info) => info.isFIFO())
      .catch(() => false);
    const relayPipeActive =
      hasPaneTtyPath &&
      outputLogExists &&
      inputPipeExists &&
      (await this.isRelayPipeActive(session));

    if (relayPipeActive) {
      if (session.output_log_path.trim() && session.input_pipe_path.trim()) {
        return session;
      }
      return terminalStore.saveSession(
        normalizeTerminalSessionRecord({
          ...session,
          input_pipe_path: inputPipePath,
          output_log_path: outputLogPath,
          updated_at: new Date().toISOString(),
        }),
      );
    }

    return this.configureSessionRuntime(
      normalizeTerminalSessionRecord({
        ...session,
        input_pipe_path: inputPipePath,
        output_log_path: outputLogPath,
      }),
    );
  }

  async createSession(
    input: CreateSessionInput,
    clientIp: string,
  ): Promise<TerminalSessionRecord> {
    await this.cleanupExpiredSessions();
    await this.assertCreateAllowed();

    const config = await this.getFeatureConfig();
    const existing = await terminalStore.listSessions();
    if (existing.length >= config.max_sessions) {
      throw new Error(`终端会话已达到上限（${config.max_sessions}）`);
    }

    const shell = await this.resolveShell(input.shell);
    const cwd = await this.resolveCwd(input.cwd);
    const cols = Math.min(400, Math.max(40, Math.floor(input.cols || 120)));
    const rows = Math.min(200, Math.max(12, Math.floor(input.rows || 32)));
    const id = uuidv4();
    const now = new Date().toISOString();
    const sessionName = this.buildSessionName(id);
    const title =
      this.sanitizeTitle(input.title) ||
      this.buildDefaultSessionTitle(existing);

    const createResult = await this.runTmux([
      "new-session",
      "-d",
      "-s",
      sessionName,
      "-x",
      String(cols),
      "-y",
      String(rows),
      "-c",
      cwd,
      shell,
    ]);
    if (createResult.code !== 0) {
      throw new Error(createResult.stderr || "tmux 会话创建失败");
    }

    const session = normalizeTerminalSessionRecord({
      id,
      title,
      status: "detached",
      created_at: now,
      updated_at: now,
      last_client_ip: clientIp,
      shell,
      cwd,
      cols,
      rows,
      resume_backend: "tmux",
      backend_session_name: sessionName,
      pane_tty_path: "",
      input_pipe_path: this.buildInputPipePath(id),
      output_log_path: this.buildOutputLogPath(id),
      expires_at: new Date(
        Date.now() + config.idle_timeout_seconds * 1000,
      ).toISOString(),
    });

    try {
      return await this.configureSessionRuntime(session);
    } catch (error) {
      await this.runTmux(["kill-session", "-t", sessionName]).catch(
        () => undefined,
      );
      await this.cleanupSessionArtifacts(session).catch(() => undefined);
      throw error;
    }
  }

  async renameSession(
    id: string,
    title: string,
  ): Promise<TerminalSessionRecord | null> {
    const session = await terminalStore.getSession(id);
    if (!session) return null;

    const sanitizedTitle = this.sanitizeTitle(title);
    if (!sanitizedTitle) {
      throw new Error("会话名称不能为空");
    }

    return terminalStore.saveSession(
      normalizeTerminalSessionRecord({
        ...session,
        title: sanitizedTitle,
        updated_at: new Date().toISOString(),
      }),
    );
  }

  async killSession(id: string): Promise<void> {
    const session = await terminalStore.getSession(id);
    if (!session) return;
    await this.closeInputPipeWriter(id);
    await this.runTmux([
      "kill-session",
      "-t",
      session.backend_session_name,
    ]).catch(() => undefined);
    await this.cleanupSessionArtifacts(session);
    await terminalStore.deleteSession(id);
  }

  private async markSessionDetached(
    sessionId: string,
  ): Promise<TerminalSessionRecord | null> {
    const session = await terminalStore.getSession(sessionId);
    if (!session) return null;
    return this.refreshSessionExpiry(
      normalizeTerminalSessionRecord({
        ...session,
        status: "detached",
        updated_at: new Date().toISOString(),
        last_detached_at: new Date().toISOString(),
      }),
    );
  }

  async createAttachment(
    sessionId: string,
    clientIp: string,
  ): Promise<TerminalAttachmentRecord> {
    const session = await this.getSession(sessionId);
    if (!session) {
      throw new Error("终端会话不存在或已失效");
    }

    const status = await this.getRuntimeStatus();
    if (!status.enabled) {
      throw new Error("网页终端功能尚未启用");
    }
    if (!status.tmuxAvailable) {
      throw new Error("未检测到 tmux，无法附着终端会话");
    }

    // Keep the existing relay/log pipeline when it is already healthy.
    // Rebuilding it on every attach can drop the very first echoed bytes,
    // which makes the initial HTTP poll appear to "miss" early keystrokes.
    const runtimeSession = await this.ensureSessionRuntime(session);
    const now = Date.now();
    await terminalStore.saveSession(
      normalizeTerminalSessionRecord({
        ...runtimeSession,
        status: "attached",
        updated_at: new Date(now).toISOString(),
        last_attached_at: new Date(now).toISOString(),
        last_client_ip: clientIp,
        expires_at: new Date(
          now + (await this.getFeatureConfig()).idle_timeout_seconds * 1000,
        ).toISOString(),
      }),
    );

    return terminalStore.saveAttachment(
      normalizeTerminalAttachmentRecord({
        id: uuidv4(),
        session_id: runtimeSession.id,
        transport: "http-polling",
        created_at: new Date(now).toISOString(),
        updated_at: new Date(now).toISOString(),
        expires_at: new Date(
          now + DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS * 1000,
        ).toISOString(),
      }),
      DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS,
    );
  }

  async detachAttachment(attachmentId: string): Promise<void> {
    const attachment = await terminalStore.getAttachment(attachmentId);
    if (!attachment) return;
    await terminalStore.deleteAttachment(attachmentId);
    const remaining = await terminalStore.listAttachmentIdsForSession(
      attachment.session_id,
    );
    if (remaining.length === 0) {
      await this.markSessionDetached(attachment.session_id);
    }
  }

  private async openInputPipeWriter(
    session: TerminalSessionRecord,
  ): Promise<FileHandle> {
    const deadline = Date.now() + 5_000;
    let lastError: unknown = null;

    while (Date.now() < deadline) {
      try {
        return await open(session.input_pipe_path, INPUT_PIPE_OPEN_FLAGS);
      } catch (error) {
        lastError = error;
        if ((error as NodeJS.ErrnoException | null)?.code !== "ENXIO") {
          throw error;
        }
        await sleep(50);
      }
    }

    throw lastError instanceof Error
      ? lastError
      : new Error("终端输入管道尚未就绪");
  }

  private async getInputPipeWriter(
    session: TerminalSessionRecord,
  ): Promise<FileHandle> {
    const cached = this.inputPipeWriterHandles.get(session.id);
    if (cached) {
      return cached;
    }

    const writerPromise = this.openInputPipeWriter(session).catch((error) => {
      this.inputPipeWriterHandles.delete(session.id);
      throw error;
    });
    this.inputPipeWriterHandles.set(session.id, writerPromise);
    return writerPromise;
  }

  private async writeToInputPipe(
    session: TerminalSessionRecord,
    data: Buffer,
  ): Promise<void> {
    const writer = await this.getInputPipeWriter(session);
    let offset = 0;

    while (offset < data.length) {
      const { bytesWritten } = await writer.write(
        data,
        offset,
        data.length - offset,
        null,
      );
      if (bytesWritten <= 0) {
        throw new Error("终端输入写入被中断");
      }
      offset += bytesWritten;
    }
  }

  async sendInput(attachmentId: string, dataBase64: string): Promise<void> {
    const attachment = await terminalStore.getAttachment(attachmentId);
    if (!attachment) {
      throw new Error("终端附着已失效");
    }
    const session = await terminalStore.getSession(attachment.session_id);
    if (!session) {
      throw new Error("终端会话不存在或已失效");
    }

    const data = Buffer.from(String(dataBase64 || ""), "base64");
    if (data.length === 0) return;

    let runtimeSession = session.input_pipe_path.trim()
      ? session
      : await this.ensureSessionRuntime(session);
    try {
      await this.writeToInputPipe(runtimeSession, data);
    } catch (error) {
      await this.closeInputPipeWriter(runtimeSession.id);
      const confirmedSession = await this.getSession(runtimeSession.id);
      if (!confirmedSession) {
        throw new Error("终端会话不存在或已失效");
      }
      runtimeSession = await this.configureSessionRuntime(confirmedSession);
      try {
        await this.writeToInputPipe(runtimeSession, data);
      } catch (retryError) {
        throw new Error(this.formatIoError("终端输入发送失败", retryError));
      }
      console.warn(
        "[terminal] input pipe write recovered after runtime refresh",
        {
          sessionId: runtimeSession.id,
          error:
            error instanceof Error ? error.message : String(error ?? "unknown"),
        },
      );
    }

    await this.touchSessionActivity(runtimeSession);
  }

  async resizeAttachment(
    attachmentId: string,
    cols: number,
    rows: number,
  ): Promise<TerminalSessionRecord> {
    const attachment = await terminalStore.refreshAttachment(attachmentId);
    if (!attachment) {
      throw new Error("终端附着已失效");
    }
    const session = await this.getSession(attachment.session_id);
    if (!session) {
      throw new Error("终端会话不存在或已失效");
    }

    const nextCols = Math.min(
      400,
      Math.max(40, Math.floor(cols || session.cols)),
    );
    const nextRows = Math.min(
      200,
      Math.max(12, Math.floor(rows || session.rows)),
    );

    const resizeResult = await this.runTmux([
      "resize-window",
      "-t",
      session.backend_session_name,
      "-x",
      String(nextCols),
      "-y",
      String(nextRows),
    ]);
    if (resizeResult.code !== 0) {
      throw new Error(resizeResult.stderr || "终端尺寸调整失败");
    }

    return this.refreshSessionExpiry(
      normalizeTerminalSessionRecord({
        ...session,
        cols: nextCols,
        rows: nextRows,
        updated_at: new Date().toISOString(),
      }),
    );
  }

  private async readOutputChunk(
    session: TerminalSessionRecord,
    requestedCursor: number,
  ): Promise<TerminalOutputChunk | null> {
    const outputLogPath = session.output_log_path.trim();
    const updatedAt = new Date().toISOString();

    if (!outputLogPath) {
      return requestedCursor > 0
        ? {
            cursor: 0,
            data_base64: "",
            reset: true,
            updatedAt,
          }
        : null;
    }

    const fileInfo = await stat(outputLogPath).catch(() => null);
    if (!fileInfo?.isFile()) {
      return requestedCursor > 0
        ? {
            cursor: 0,
            data_base64: "",
            reset: true,
            updatedAt,
          }
        : null;
    }

    const safeCursor =
      requestedCursor > fileInfo.size ? 0 : Math.max(0, requestedCursor);
    const reset = safeCursor !== requestedCursor;

    if (safeCursor >= fileInfo.size) {
      return reset
        ? {
            cursor: safeCursor,
            data_base64: "",
            reset,
            updatedAt,
          }
        : null;
    }

    const bytesToRead = Math.min(
      fileInfo.size - safeCursor,
      TERMINAL_STREAM_CHUNK_MAX_BYTES,
    );
    const handle = await open(outputLogPath, "r");

    try {
      const buffer = Buffer.alloc(bytesToRead);
      const { bytesRead } = await handle.read(
        buffer,
        0,
        bytesToRead,
        safeCursor,
      );
      if (bytesRead <= 0) {
        return reset
          ? {
              cursor: safeCursor,
              data_base64: "",
              reset,
              updatedAt,
            }
          : null;
      }

      return {
        cursor: safeCursor + bytesRead,
        data_base64: buffer.subarray(0, bytesRead).toString("base64"),
        reset,
        updatedAt,
      };
    } finally {
      await handle.close().catch(() => undefined);
    }
  }

  async waitForOutput(
    attachmentId: string,
    cursor = 0,
    timeoutMs = DEFAULT_TERMINAL_POLL_TIMEOUT_MS,
  ): Promise<{ changed: boolean; chunk: TerminalOutputChunk | null }> {
    const attachment = await terminalStore.refreshAttachment(attachmentId);
    if (!attachment) {
      throw new Error("终端附着已失效");
    }

    const session = await this.getSession(attachment.session_id);
    if (!session) {
      throw new Error("终端会话不存在或已失效");
    }

    const runtimeSession = await this.ensureSessionRuntime(session);
    const requestedCursor = parseOutputCursor(cursor);
    const deadline = Date.now() + Math.min(20_000, Math.max(1_000, timeoutMs));

    while (Date.now() < deadline) {
      const chunk = await this.readOutputChunk(runtimeSession, requestedCursor);
      if (chunk) {
        return { changed: true, chunk };
      }
      await sleep(DEFAULT_TERMINAL_POLL_INTERVAL_MS);
    }

    return { changed: false, chunk: null };
  }
}

export const terminalManager = new TerminalManager();
