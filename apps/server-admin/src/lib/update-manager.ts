import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { dataPath } from "./AppDirManager";
import { redis } from "./redis";
import { APP_GITHUB_URL, APP_LOCAL_VERSION } from "./app-version";
import { waitForProcessExit } from "./runtime";

const OTA_LATEST_URL = "https://ota.fnknock.cn/latest.json";
const UPDATE_PENDING_KEY = "fn_knock:update:pending";
const UPDATE_CONFIRM_KEY = "fn_knock:update:confirm";
const UPDATE_PENDING_TTL_SECONDS = 7 * 24 * 60 * 60;
const UPDATE_CONFIRM_TTL_SECONDS = 7 * 24 * 60 * 60;

type DownloadStatus = "idle" | "downloading" | "verifying" | "downloaded" | "installing" | "error";

type OtaLatestManifest = {
  version: string;
  update_available: boolean;
  force_update: boolean;
  download_url: string;
  sha256: string;
  release_notes: string;
};

type UpdatePendingPayload = {
  targetVersion: string;
  requestedAt: string;
};

type UpdateConfirmPayload = {
  version: string;
  completedAt: string;
};

type DownloadState = {
  status: DownloadStatus;
  percent: number;
  downloadedBytes: number;
  totalBytes: number | null;
  error: string | null;
  targetVersion: string | null;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

const normalizeVersion = (version: string): number[] => {
  return version
    .trim()
    .split(".")
    .map((part) => {
      const match = part.match(/\d+/);
      return match ? Number.parseInt(match[0], 10) : 0;
    });
};

const compareVersion = (a: string, b: string): number => {
  const pa = normalizeVersion(a);
  const pb = normalizeVersion(b);
  const max = Math.max(pa.length, pb.length, 3);
  for (let i = 0; i < max; i += 1) {
    const av = pa[i] ?? 0;
    const bv = pb[i] ?? 0;
    if (av > bv) return 1;
    if (av < bv) return -1;
  }
  return 0;
};

const toErrorMessage = (error: unknown, fallback: string): string => {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return fallback;
};

const parseManifest = (value: unknown): OtaLatestManifest => {
  if (!isRecord(value)) {
    throw new Error("更新信息格式错误");
  }
  const version = typeof value.version === "string" ? value.version.trim() : "";
  const updateAvailable = value.update_available;
  const forceUpdate = value.force_update;
  const downloadUrl = typeof value.download_url === "string" ? value.download_url.trim() : "";
  const sha256 = typeof value.sha256 === "string" ? value.sha256.trim().toLowerCase() : "";
  const releaseNotes = typeof value.release_notes === "string" ? value.release_notes : "";
  if (!version) {
    throw new Error("更新信息缺少 version");
  }
  if (typeof updateAvailable !== "boolean") {
    throw new Error("更新信息缺少 update_available");
  }
  if (typeof forceUpdate !== "boolean") {
    throw new Error("更新信息缺少 force_update");
  }
  if (!downloadUrl) {
    throw new Error("更新信息缺少 download_url");
  }
  if (!/^[a-f0-9]{64}$/.test(sha256)) {
    throw new Error("更新信息 sha256 无效");
  }
  return {
    version,
    update_available: updateAvailable,
    force_update: forceUpdate,
    download_url: downloadUrl,
    sha256,
    release_notes: releaseNotes,
  };
};

export class UpdateManager {
  private readonly updatesDir = path.join(dataPath, "updates");
  private readonly packageDownloadDir = path.join("/tmp", "fn-knock-updates");
  private readonly installLogPath = path.join(dataPath, "updates", "install.log");
  private readonly installEnvPath = path.join(dataPath, "updates", "install.env");
  private latestManifest: OtaLatestManifest | null = null;
  private updateEnabled = false;
  private hasUpdate = false;
  private forceUpdate = false;
  private lastCheckedAt: number | null = null;
  private checkError: string | null = null;
  private checkPromise: Promise<void> | null = null;
  private downloadPromise: Promise<void> | null = null;
  private downloadedPath: string | null = null;
  private downloadedSha256: string | null = null;
  private confirmedPendingOnBoot = false;
  private pendingConfirmPromise: Promise<void> | null = null;
  private downloadState: DownloadState = {
    status: "idle",
    percent: 0,
    downloadedBytes: 0,
    totalBytes: null,
    error: null,
    targetVersion: null,
  };

  constructor() {
    if (!fs.existsSync(this.updatesDir)) {
      fs.mkdirSync(this.updatesDir, { recursive: true });
    }
    if (!fs.existsSync(this.packageDownloadDir)) {
      fs.mkdirSync(this.packageDownloadDir, { recursive: true });
    }
  }

  private getLatestWithTimestamp(): string {
    const url = new URL(OTA_LATEST_URL);
    url.searchParams.set("t", `${Date.now()}`);
    return url.toString();
  }

  private resetDownloadState() {
    this.downloadState = {
      status: "idle",
      percent: 0,
      downloadedBytes: 0,
      totalBytes: null,
      error: null,
      targetVersion: null,
    };
    this.downloadedPath = null;
    this.downloadedSha256 = null;
  }

  private setDownloadError(message: string) {
    this.downloadState = {
      ...this.downloadState,
      status: "error",
      error: message,
    };
  }

  private async fetchManifestFromRemote(): Promise<OtaLatestManifest> {
    const res = await fetch(this.getLatestWithTimestamp(), {
      signal: AbortSignal.timeout(8000),
      headers: {
        Accept: "application/json",
        "Cache-Control": "no-cache",
        Pragma: "no-cache",
      },
    });
    if (!res.ok) {
      throw new Error(`更新检查失败: HTTP ${res.status}`);
    }
    const payload = await res.json().catch(() => null);
    return parseManifest(payload);
  }

  private async computeFileSha256(filePath: string): Promise<string> {
    const hash = createHash("sha256");
    const stream = fs.createReadStream(filePath);
    for await (const chunk of stream) {
      hash.update(chunk);
    }
    return hash.digest("hex");
  }

  private buildPackagePath(version: string) {
    return path.join(this.packageDownloadDir, `fn-knock-${version}.fpk`);
  }

  private resolveInstallPort(envKeys: string[], fallback: string): string {
    for (const key of envKeys) {
      const raw = process.env[key];
      if (!raw) continue;
      const trimmed = raw.trim();
      if (!trimmed) continue;
      const port = Number.parseInt(trimmed, 10);
      if (Number.isInteger(port) && port >= 1 && port <= 65535) {
        return `${port}`;
      }
    }
    return fallback;
  }

  private buildInstallEnvContent(): string {
    const backendPort = this.resolveInstallPort(["wizard_backend_port", "BACKEND_PORT"], "7998");
    const authPort = this.resolveInstallPort(["wizard_auth_port", "AUTH_PORT"], "7997");
    const goBackendPort = this.resolveInstallPort(["wizard_go_backend_port", "GO_BACKEND_PORT"], "7996");
    const goReproxyPort = this.resolveInstallPort(
      ["wizard_go_reproxy_port", "GO_REPROXY_PORT", "TRIM_SERVICE_PORT"],
      "7999",
    );

    return [
      `wizard_backend_port=${backendPort}`,
      `wizard_auth_port=${authPort}`,
      `wizard_go_backend_port=${goBackendPort}`,
      `wizard_go_reproxy_port=${goReproxyPort}`,
      `BACKEND_PORT=${backendPort}`,
      `AUTH_PORT=${authPort}`,
      `GO_BACKEND_PORT=${goBackendPort}`,
      `GO_REPROXY_PORT=${goReproxyPort}`,
      "",
    ].join("\n");
  }

  private async ensureConfirmByPending() {
    if (this.confirmedPendingOnBoot) return;
    if (this.pendingConfirmPromise) {
      return this.pendingConfirmPromise;
    }
    this.pendingConfirmPromise = (async () => {
      const raw = await redis.get(UPDATE_PENDING_KEY);
      if (!raw) {
        this.confirmedPendingOnBoot = true;
        return;
      }
      try {
        const parsed = JSON.parse(raw) as UpdatePendingPayload;
        const targetVersion = parsed?.targetVersion?.trim();
        if (!targetVersion) {
          this.confirmedPendingOnBoot = true;
          return;
        }
        if (compareVersion(targetVersion, APP_LOCAL_VERSION) === 0) {
          const confirmPayload: UpdateConfirmPayload = {
            version: targetVersion,
            completedAt: new Date().toISOString(),
          };
          await redis.set(UPDATE_CONFIRM_KEY, JSON.stringify(confirmPayload), "EX", UPDATE_CONFIRM_TTL_SECONDS);
          await redis.del(UPDATE_PENDING_KEY);
        }
      } catch {
        // ignore malformed pending payload
      } finally {
        this.confirmedPendingOnBoot = true;
      }
    })().finally(() => {
      this.pendingConfirmPromise = null;
    });
    return this.pendingConfirmPromise;
  }

  async consumeConfirmMessage(): Promise<UpdateConfirmPayload | null> {
    await this.ensureConfirmByPending();
    const raw = await redis.get(UPDATE_CONFIRM_KEY);
    if (!raw) return null;
    await redis.del(UPDATE_CONFIRM_KEY);
    try {
      const parsed = JSON.parse(raw) as UpdateConfirmPayload;
      if (typeof parsed?.version === "string" && parsed.version.trim()) {
        return parsed;
      }
    } catch {
      // ignore malformed confirm payload
    }
    return null;
  }

  async prepareOnBoot() {
    await this.ensureConfirmByPending();
  }

  async checkNow(reason = "manual"): Promise<void> {
    if (this.checkPromise) {
      return this.checkPromise;
    }
    this.checkPromise = this.checkNowInternal(reason).finally(() => {
      this.checkPromise = null;
    });
    return this.checkPromise;
  }

  private async checkNowInternal(reason: string): Promise<void> {
    try {
      const manifest = await this.fetchManifestFromRemote();
      this.latestManifest = manifest;
      this.lastCheckedAt = Date.now();
      this.checkError = null;
      this.updateEnabled = manifest.update_available === true;
      this.hasUpdate = this.updateEnabled && compareVersion(manifest.version, APP_LOCAL_VERSION) > 0;
      this.forceUpdate = this.hasUpdate && manifest.force_update;

      // 来源变更后，清理过时的下载状态
      if (
        this.downloadState.targetVersion &&
        this.downloadState.targetVersion !== manifest.version &&
        this.downloadState.status !== "downloading" &&
        this.downloadState.status !== "installing"
      ) {
        this.resetDownloadState();
      }
    } catch (error) {
      this.checkError = toErrorMessage(error, "更新检查失败");
      this.lastCheckedAt = Date.now();
      console.error(`[update] check failed (${reason}):`, error);
    }
  }

  async triggerDownload(): Promise<void> {
    if (this.downloadState.status === "downloading" || this.downloadState.status === "verifying") {
      return;
    }
    if (this.downloadPromise) {
      return this.downloadPromise;
    }

    if (!this.latestManifest) {
      await this.checkNow("download-bootstrap");
    }
    if (!this.latestManifest) {
      throw new Error("尚未获取到更新信息");
    }
    if (!this.updateEnabled) {
      throw new Error("更新功能当前未启用");
    }
    if (!this.hasUpdate) {
      throw new Error("当前已是最新版本");
    }

    const targetVersion = this.latestManifest.version;
    const targetSha256 = this.latestManifest.sha256;
    const targetPath = this.buildPackagePath(targetVersion);

    if (
      this.downloadState.status === "downloaded" &&
      this.downloadedPath === targetPath &&
      this.downloadedSha256 === targetSha256 &&
      fs.existsSync(targetPath)
    ) {
      return;
    }

    this.downloadPromise = this.downloadInternal(this.latestManifest, targetPath)
      .catch((error) => {
        console.error("[update] download failed:", error);
      })
      .finally(() => {
        this.downloadPromise = null;
      });
  }

  private async downloadInternal(manifest: OtaLatestManifest, targetPath: string): Promise<void> {
    const tempPath = `${targetPath}.tmp`;
    const stream = fs.createWriteStream(tempPath);
    this.downloadState = {
      status: "downloading",
      percent: 0,
      downloadedBytes: 0,
      totalBytes: null,
      error: null,
      targetVersion: manifest.version,
    };

    try {
      const response = await fetch(manifest.download_url, {
        signal: AbortSignal.timeout(300_000),
        headers: {
          "Cache-Control": "no-cache",
          Pragma: "no-cache",
        },
      });
      if (!response.ok) {
        throw new Error(`下载失败: HTTP ${response.status}`);
      }
      const totalHeader = response.headers.get("content-length");
      const totalBytes = totalHeader ? Number.parseInt(totalHeader, 10) : 0;
      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("下载失败: 响应流不可读");
      }

      let loadedBytes = 0;
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (!value) continue;
        loadedBytes += value.length;
        stream.write(value);
        const percent = totalBytes > 0 ? Math.min(100, Math.round((loadedBytes / totalBytes) * 100)) : 0;
        this.downloadState = {
          ...this.downloadState,
          status: "downloading",
          percent,
          downloadedBytes: loadedBytes,
          totalBytes: totalBytes > 0 ? totalBytes : null,
          error: null,
        };
      }
      await new Promise<void>((resolve, reject) => {
        stream.end(() => resolve());
        stream.on("error", reject);
      });

      this.downloadState = {
        ...this.downloadState,
        status: "verifying",
        percent: 100,
      };
      const sha256 = (await this.computeFileSha256(tempPath)).toLowerCase();
      if (sha256 !== manifest.sha256.toLowerCase()) {
        throw new Error(`校验失败: 期望 ${manifest.sha256}，实际 ${sha256}`);
      }

      fs.renameSync(tempPath, targetPath);
      this.downloadedPath = targetPath;
      this.downloadedSha256 = sha256;
      this.downloadState = {
        status: "downloaded",
        percent: 100,
        downloadedBytes: fs.statSync(targetPath).size,
        totalBytes: fs.statSync(targetPath).size,
        error: null,
        targetVersion: manifest.version,
      };
    } catch (error) {
      try {
        if (fs.existsSync(tempPath)) {
          fs.unlinkSync(tempPath);
        }
      } catch {
        // ignore cleanup error
      }
      this.setDownloadError(toErrorMessage(error, "下载失败"));
      throw error;
    }
  }

  async triggerInstall(): Promise<void> {
    if (this.downloadState.status === "installing") {
      return;
    }
    if (!this.latestManifest || !this.updateEnabled || !this.hasUpdate) {
      throw new Error("当前没有可安装更新");
    }
    if (this.downloadState.status !== "downloaded" || !this.downloadedPath) {
      throw new Error("请先完成更新包下载并校验");
    }
    if (!fs.existsSync(this.downloadedPath)) {
      this.resetDownloadState();
      throw new Error("更新包不存在，请重新下载");
    }

    const currentSha256 = (await this.computeFileSha256(this.downloadedPath)).toLowerCase();
    if (currentSha256 !== this.latestManifest.sha256.toLowerCase()) {
      this.resetDownloadState();
      throw new Error("更新包校验失败，请重新下载");
    }

    const pendingPayload: UpdatePendingPayload = {
      targetVersion: this.latestManifest.version,
      requestedAt: new Date().toISOString(),
    };
    await redis.set(UPDATE_PENDING_KEY, JSON.stringify(pendingPayload), "EX", UPDATE_PENDING_TTL_SECONDS);

    this.downloadState = {
      ...this.downloadState,
      status: "installing",
      error: null,
    };

    const scriptPath = path.join(this.updatesDir, "apply-update.sh");
    const escapedPath = this.downloadedPath.replace(/"/g, '\\"');
    const envContent = this.buildInstallEnvContent();
    fs.writeFileSync(this.installEnvPath, envContent, "utf-8");

    const escapedScriptPath = scriptPath.replace(/"/g, '\\"');
    const escapedEnvPath = this.installEnvPath.replace(/"/g, '\\"');
    const escapedLogPath = this.installLogPath.replace(/"/g, '\\"');
    const script = `#!/bin/sh
set -eu
sleep 2

if [ "$(id -u)" -ne 0 ]; then
  if command -v sudo >/dev/null 2>&1; then
    exec sudo -n /bin/sh "$0" "$@"
  fi
  echo "root privileges are required for update installation" >&2
  exit 1
fi

appcenter-cli stop fn-knock || true
appcenter-cli uninstall fn-knock || true
mkdir -p /tmp/appcenter
appcenter-cli install-fpk "${escapedPath}" --env "${escapedEnvPath}"
appcenter-cli start fn-knock
`;
    fs.writeFileSync(scriptPath, script, "utf-8");
    fs.chmodSync(scriptPath, 0o755);

    const launcher = `if command -v setsid >/dev/null 2>&1; then
  nohup setsid /bin/sh "${escapedScriptPath}" > "${escapedLogPath}" 2>&1 < /dev/null &
else
  nohup /bin/sh "${escapedScriptPath}" > "${escapedLogPath}" 2>&1 < /dev/null &
fi`;
    const run = spawn("/bin/sh", ["-c", launcher], {
      stdout: "ignore",
      stderr: "ignore",
    });
    const exitCode = await waitForProcessExit(run);
    if (exitCode !== 0) {
      this.setDownloadError("启动更新安装流程失败");
      throw new Error("启动更新安装流程失败");
    }
  }

  async getStatus() {
    await this.ensureConfirmByPending();
    return {
      githubUrl: APP_GITHUB_URL,
      localVersion: APP_LOCAL_VERSION,
      latest: this.latestManifest,
      updateEnabled: this.updateEnabled,
      hasUpdate: this.hasUpdate,
      forceUpdate: this.forceUpdate,
      check: {
        lastCheckedAt: this.lastCheckedAt,
        error: this.checkError,
      },
      download: this.downloadState,
    };
  }
}

export const updateManager = new UpdateManager();
