import path from 'path';
import fs from 'fs';
import { spawn } from "node:child_process";
import { dataPath } from './AppDirManager';
import { waitForProcessExit } from "./runtime";

type DownloadState = 'idle' | 'downloading' | 'completed' | 'error';

type Status = {
  supported: boolean;
  platform: 'darwin-arm64' | 'linux-amd64' | 'linux-arm64' | 'linux-arm' | 'unsupported';
  downloaded: boolean;
  progress: {
    status: DownloadState;
    percent: number;
    error?: string;
  };
};

const DATA_DIR = dataPath;
const FRP_DIR = path.join(DATA_DIR, 'frp');
const TAR_PATH = path.join(FRP_DIR, 'frp.tar.gz');
const VERSION = '0.67.0';

if (!fs.existsSync(DATA_DIR)) fs.mkdirSync(DATA_DIR, { recursive: true });
if (!fs.existsSync(FRP_DIR)) fs.mkdirSync(FRP_DIR, { recursive: true });

class FrpManager {
  private abortController: AbortController | null = null;
  private progress: Status['progress'] = { status: 'idle', percent: 0 };

  private archiveNameForPlatform(platform: Status['platform']): string | null {
    switch (platform) {
      case 'linux-amd64':
        return `frp_${VERSION}_linux_amd64`;
      case 'linux-arm64':
        return `frp_${VERSION}_linux_arm64`;
      case 'linux-arm':
        return `frp_${VERSION}_linux_arm`;
      case 'darwin-arm64':
        return `frp_${VERSION}_darwin_arm64`;
      default:
        return null;
    }
  }

  private dirNameForPlatform(platform: Status['platform']): string | null {
    return this.archiveNameForPlatform(platform);
  }

  private extractedDir(platform: Status['platform']): string | null {
    const dirName = this.dirNameForPlatform(platform);
    if (!dirName) return null;
    return path.join(FRP_DIR, dirName);
  }

  private binaryPath(kind: 'frpc' | 'frps', platform: Status['platform']): string | null {
    const base = this.extractedDir(platform);
    if (!base) return null;
    return path.join(base, kind);
  }

  private detectPlatform(): Status['platform'] {
    const p = process.platform;
    const a = process.arch;
    if (p === 'darwin' && a === 'arm64') return 'darwin-arm64';
    if (p === 'linux' && a === 'x64') return 'linux-amd64';
    if (p === 'linux' && a === 'arm64') return 'linux-arm64';
    if (p === 'linux' && a === 'arm') return 'linux-arm';
    return 'unsupported';
  }

  private urlCandidatesForPlatform(platform: Status['platform']): string[] {
    const mirrorBase = 'https://hongkong-10009423.cos.ap-shanghai.myqcloud.com/frp/';
    const ghBase = 'https://github.com/fatedier/frp/releases/download/v0.67.0';
    const archiveName = this.archiveNameForPlatform(platform);
    if (!archiveName) return [];

    return [
      `${mirrorBase}/${archiveName}.tar.gz`,
      `${ghBase}/${archiveName}.tar.gz`
    ];
  }

  getStatus(): Status {
    const platform = this.detectPlatform();
    const supported = platform !== 'unsupported';
    const frpc = this.binaryPath('frpc', platform);
    const frps = this.binaryPath('frps', platform);
    const downloaded = !!(frpc && fs.existsSync(frpc)) || !!(frps && fs.existsSync(frps));
    return {
      supported,
      platform,
      downloaded,
      progress: this.progress
    };
  }

  private async extractAndPrepare(platform: Status['platform']): Promise<void> {
    const targetDir = this.extractedDir(platform);
    if (!targetDir) throw new Error('当前平台不受支持');
    if (!fs.existsSync(TAR_PATH)) throw new Error('FRP 安装包缺失');
    if (fs.existsSync(targetDir)) {
      fs.rmSync(targetDir, { recursive: true, force: true });
    }
    const proc = spawn("tar", ["-xzf", TAR_PATH, "-C", FRP_DIR], {
      stdio: "ignore",
    });
    const code = await waitForProcessExit(proc);
    if (code !== 0) {
      throw new Error(`解压失败，退出码 ${code}`);
    }
    const frpc = path.join(targetDir, 'frpc');
    const frps = path.join(targetDir, 'frps');
    if (fs.existsSync(frpc)) fs.chmodSync(frpc, 0o755);
    if (fs.existsSync(frps)) fs.chmodSync(frps, 0o755);
  }

  async startDownload(): Promise<void> {
    if (this.progress.status === 'downloading') return;
    const platform = this.detectPlatform();
    const candidates = this.urlCandidatesForPlatform(platform);
    if (!candidates.length) {
      this.progress = { status: 'error', percent: 0, error: '当前平台不受支持' };
      return;
    }
    this.abortController = new AbortController();
    this.progress = { status: 'downloading', percent: 0 };
    const tempPath = TAR_PATH + '.tmp';
    try {
      let lastErr: string | undefined;
      let succeeded = false;
      for (const url of candidates) {
        try {
          const res = await fetch(url, { signal: this.abortController.signal });
          if (!res.ok) {
            lastErr = `HTTP ${res.status} ${res.statusText}`;
            continue;
          }
          const totalHeader = res.headers.get('content-length');
          const total = totalHeader ? parseInt(totalHeader, 10) : 0;
          let loaded = 0;
          const reader = res.body?.getReader();
          if (!reader) throw new Error('下载响应体不可读');
          const stream = fs.createWriteStream(tempPath);
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            if (value) {
              loaded += value.length;
              stream.write(value);
              if (total > 0) {
                const percent = Math.min(100, Math.round((loaded / total) * 100));
                this.progress = { status: 'downloading', percent };
              }
            }
          }
          stream.end();
          await new Promise<void>((resolve, reject) => {
            stream.on('finish', () => resolve());
            stream.on('error', reject);
          });
          succeeded = true;
          break;
        } catch (e: any) {
          lastErr = e?.message || '连接失败';
          continue;
        }
      }
      if (!succeeded) {
        this.progress = { status: 'error', percent: 0, error: `下载失败：${lastErr || '未知错误'}` };
        this.abortController = null;
        if (fs.existsSync(tempPath)) fs.unlinkSync(tempPath);
        return;
      }
      fs.renameSync(tempPath, TAR_PATH);
      await this.extractAndPrepare(platform);
      this.progress = { status: 'completed', percent: 100 };
    } catch (e: any) {
      this.progress = { status: 'error', percent: 0, error: e?.message || '未知错误' };
      if (fs.existsSync(tempPath)) fs.unlinkSync(tempPath);
    } finally {
      this.abortController = null;
    }
  }

  cancelDownload(): void {
    if (this.abortController) {
      try {
        this.abortController.abort();
      } catch {}
      this.abortController = null;
    }
    try {
      const tmp = TAR_PATH + '.tmp';
      if (fs.existsSync(tmp)) fs.unlinkSync(tmp);
    } catch {}
    this.progress = { status: 'idle', percent: 0, error: '下载已取消' };
  }

  async delete(): Promise<void> {
    if (fs.existsSync(TAR_PATH)) fs.unlinkSync(TAR_PATH);
    const platform = this.detectPlatform();
    const dir = this.extractedDir(platform);
    if (dir && fs.existsSync(dir)) fs.rmSync(dir, { recursive: true, force: true });
    this.progress = { status: 'idle', percent: 0 };
  }

  getExecutable(kind: 'frpc' | 'frps' = 'frpc'): string {
    const platform = this.detectPlatform();
    const bin = this.binaryPath(kind, platform);
    if (!bin || !fs.existsSync(bin)) {
      throw new Error('FRP 未初始化，请先下载');
    }
    return bin;
  }
}

export const frpManager = new FrpManager();
