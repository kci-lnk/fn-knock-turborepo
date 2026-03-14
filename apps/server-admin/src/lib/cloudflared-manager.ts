import path from 'path';
import fs from 'fs';
import { spawnSync } from "node:child_process";
import { dataPath } from './AppDirManager';

type DownloadState = 'idle' | 'downloading' | 'completed' | 'error';

type Status = {
  supported: boolean;
  platform: 'darwin' | 'linux-amd64' | 'linux-arm64' | 'linux-arm' | 'unsupported';
  downloaded: boolean;
  progress: {
    status: DownloadState;
    percent: number;
    error?: string;
  };
};

const DATA_DIR = dataPath;
const CLOUDFLARED_DIR = path.join(DATA_DIR, 'cloudflared');
const BIN_PATH = path.join(CLOUDFLARED_DIR, 'cloudflared');

if (!fs.existsSync(DATA_DIR)) fs.mkdirSync(DATA_DIR, { recursive: true });
if (!fs.existsSync(CLOUDFLARED_DIR)) fs.mkdirSync(CLOUDFLARED_DIR, { recursive: true });

class CloudflaredManager {
  private abortController: AbortController | null = null;
  private progress: Status['progress'] = { status: 'idle', percent: 0 };

  private downloadUrlForPlatform(platform: Status['platform']): string | null {
    const mirrorBase = 'https://hongkong-10009423.cos.ap-shanghai.myqcloud.com';
    switch (platform) {
      case 'linux-amd64':
        return `${mirrorBase}/cloudflared-linux-amd64`;
      case 'linux-arm64':
        return `${mirrorBase}/cloudflared-linux-arm64`;
      case 'linux-arm':
        return `${mirrorBase}/cloudflared-linux-arm`;
      default:
        return null;
    }
  }

  private detectPlatform(): Status['platform'] {
    const p = process.platform;
    const a = process.arch;
    if (p === 'darwin') return 'darwin';
    if (p === 'linux' && a === 'x64') return 'linux-amd64';
    if (p === 'linux' && a === 'arm64') return 'linux-arm64';
    if (p === 'linux' && a === 'arm') return 'linux-arm';
    return 'unsupported';
  }

  getStatus(): Status {
    const platform = this.detectPlatform();
    const supported = platform !== 'unsupported';
    let downloaded = false;
    
    if (platform === 'darwin') {
      try {
        const proc = spawnSync("which", ["cloudflared"], { encoding: "utf-8" });
        downloaded = proc.status === 0;
      } catch {
        downloaded = false;
      }
    } else if (platform === 'linux-amd64' || platform === 'linux-arm64' || platform === 'linux-arm') {
      downloaded = fs.existsSync(BIN_PATH);
    }

    return {
      supported,
      platform,
      downloaded,
      progress: this.progress
    };
  }

  async startDownload(): Promise<void> {
    if (this.progress.status === 'downloading') return;
    const platform = this.detectPlatform();
    
    if (platform === 'darwin') {
        this.progress = { status: 'error', percent: 0, error: 'MAC平台暂不支持自动下载应用，请手动通过brew install cloudflared安装。' };
        return;
    }
    
    const url = this.downloadUrlForPlatform(platform);
    if (!url) {
      this.progress = { status: 'error', percent: 0, error: '当前平台不受支持' };
      return;
    }
    this.abortController = new AbortController();
    this.progress = { status: 'downloading', percent: 0 };
    const tempPath = BIN_PATH + '.tmp';
    
    try {
      const res = await fetch(url, { signal: this.abortController.signal });
      if (!res.ok) {
        throw new Error(`HTTP ${res.status} ${res.statusText}`);
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
      
      fs.renameSync(tempPath, BIN_PATH);
      fs.chmodSync(BIN_PATH, 0o755);
      this.progress = { status: 'completed', percent: 100 };
    } catch (e: any) {
      if (e.name === 'AbortError' || e.message.includes('abort')) {
         this.progress = { status: 'idle', percent: 0, error: '下载已取消' };
      } else {
         this.progress = { status: 'error', percent: 0, error: e?.message || '未知错误' };
      }
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
      const tmp = BIN_PATH + '.tmp';
      if (fs.existsSync(tmp)) fs.unlinkSync(tmp);
    } catch {}
    this.progress = { status: 'idle', percent: 0, error: '下载已取消' };
  }

  async delete(): Promise<void> {
    const platform = this.detectPlatform();
    if (platform === 'darwin') {
        throw new Error('MAC平台请手动移除cloudflared');
    }
    if (fs.existsSync(BIN_PATH)) fs.unlinkSync(BIN_PATH);
    this.progress = { status: 'idle', percent: 0 };
  }

  getExecutable(): string {
    const platform = this.detectPlatform();
    if (platform === 'darwin') {
      try {
        const proc = spawnSync("which", ["cloudflared"], { encoding: "utf-8" });
        if (proc.status === 0) {
          return (proc.stdout || "").toString().trim() || 'cloudflared';
        }
      } catch {
        // ignore
      }
      throw new Error('Cloudflared 未安装，请先通过 brew install cloudflared 安装');
    } else if (platform === 'linux-amd64' || platform === 'linux-arm64' || platform === 'linux-arm') {
      if (fs.existsSync(BIN_PATH)) {
        return BIN_PATH;
      }
      throw new Error('Cloudflared 未初始化，请先下下载');
    }
    throw new Error('当前平台不受支持');
  }
}

export const cloudflaredManager = new CloudflaredManager();
