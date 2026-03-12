import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { IPv4, IPv6, newWithBuffer } from 'ip2region.js';
import { dataPath } from './AppDirManager';

const DATA_DIR = dataPath;
const DB_PATH_V4 = path.join(DATA_DIR, 'ip2region_v4.xdb');
const DB_PATH_V6 = path.join(DATA_DIR, 'ip2region_v6.xdb');

if (!fs.existsSync(DATA_DIR)) {
    fs.mkdirSync(DATA_DIR, { recursive: true });
}

export interface IpInfo {
    raw: string;
    country: string;
    province: string;
    city: string;
    isp: string;
}

export interface DownloadProgress {
    status: 'idle' | 'downloading' | 'completed' | 'error';
    progressV4: number; // 0-100
    progressV6: number; // 0-100
    error?: string;
}

class IpLocationService {
    private searcherV4: any = null;
    private searcherV6: any = null;
    private abortController: AbortController | null = null;

    private progress: DownloadProgress = {
        status: 'idle',
        progressV4: 0,
        progressV6: 0
    };

    constructor() {
        this.initSearchers();
    }

    /**
     * 初始化 Searcher
     * 将 xdb 文件一次性加载到内存中
     */
    private initSearchers() {
        try {
            if (!this.searcherV4 && fs.existsSync(DB_PATH_V4)) {
                const bufferV4 = fs.readFileSync(DB_PATH_V4);
                this.searcherV4 = newWithBuffer(IPv4, bufferV4);
            }

            if (!this.searcherV6 && fs.existsSync(DB_PATH_V6)) {
                const bufferV6 = fs.readFileSync(DB_PATH_V6);
                this.searcherV6 = newWithBuffer(IPv6, bufferV6);
            }

            if (this.searcherV4 && this.searcherV6 && this.progress.status !== 'downloading') {
                this.progress.status = 'completed';
            }
        } catch (error) {
            console.error("Failed to initialize IP Searchers:", error);
        }
    }

    public getStatus() {
        const hasV4 = fs.existsSync(DB_PATH_V4);
        const hasV6 = fs.existsSync(DB_PATH_V6);
        const isInitialized = hasV4 && hasV6 && this.searcherV4 !== null && this.searcherV6 !== null;

        return {
            isInitialized,
            hasV4,
            hasV6,
            progress: this.progress
        };
    }

    private async downloadFile(url: string, destPath: string, onProgress: (percent: number) => void, signal: AbortSignal): Promise<void> {
        const response = await fetch(url, { signal });

        if (!response.ok) {
            throw new Error(`Failed to fetch ${url}: ${response.statusText}`);
        }

        const contentLength = response.headers.get('content-length');
        const totalBytes = contentLength ? parseInt(contentLength, 10) : 0;

        let loadedBytes = 0;
        const reader = response.body?.getReader();
        if (!reader) throw new Error("Failed to get reader from response");

        // We download to a temp file first, then rename it
        const tempPath = destPath + '.tmp';
        const stream = fs.createWriteStream(tempPath);

        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                if (value) {
                    loadedBytes += value.length;
                    stream.write(value);
                    if (totalBytes > 0) {
                        const percent = Math.min(100, Math.round((loadedBytes / totalBytes) * 100));
                        onProgress(percent);
                    }
                }
            }
            stream.end();
            return new Promise((resolve, reject) => {
                stream.on('finish', () => {
                    fs.renameSync(tempPath, destPath);
                    resolve();
                });
                stream.on('error', reject);
            });
        } catch (error) {
            stream.end();
            if (fs.existsSync(tempPath)) {
                fs.unlinkSync(tempPath);
            }
            throw error;
        }
    }

    public async startDownload() {
        if (this.progress.status === 'downloading') {
            return;
        }

        this.abortController = new AbortController();
        this.progress = {
            status: 'downloading',
            progressV4: 0,
            progressV6: 0
        };

        const v4Url = 'https://cdn.jsdelivr.net/npm/ip2region-wr@latest/data/ip2region_v4.xdb';
        const v6Url = 'https://cdn.jsdelivr.net/npm/ip2region-wr@latest/data/ip2region_v6.xdb';

        try {
            // Download V4 and V6 concurrently
            await Promise.all([
                this.downloadFile(v4Url, DB_PATH_V4, (p) => { this.progress.progressV4 = p; }, this.abortController.signal),
                this.downloadFile(v6Url, DB_PATH_V6, (p) => { this.progress.progressV6 = p; }, this.abortController.signal)
            ]);

            this.progress.status = 'completed';
            this.progress.progressV4 = 100;
            this.progress.progressV6 = 100;

            // Re-initialize after download
            this.initSearchers();
        } catch (error: any) {
            if (error.name === 'AbortError') {
                this.progress.status = 'idle';
                this.progress.error = '下载已取消';
            } else {
                console.error("IP DB Download error:", error);
                this.progress.status = 'error';
                this.progress.error = error.message || 'Unknown download error';
            }

            // Clean up partial files
            if (fs.existsSync(DB_PATH_V4 + '.tmp')) fs.unlinkSync(DB_PATH_V4 + '.tmp');
            if (fs.existsSync(DB_PATH_V6 + '.tmp')) fs.unlinkSync(DB_PATH_V6 + '.tmp');
        } finally {
            this.abortController = null;
        }
    }

    public cancelDownload() {
        if (this.abortController) {
            this.abortController.abort();
            this.abortController = null;
        }
    }

    public async getIpLocation(ip: string): Promise<IpInfo | null> {
        try {
            const isIPv6 = ip.includes(':');
            const searcher = isIPv6 ? this.searcherV6 : this.searcherV4;

            if (!searcher) {
                return null;
            }

            const region = await searcher.search(ip);

            if (!region) return null;

            const parts = region.split('|');

            let country = parts[0] || '';
            let province = parts[2] || '';
            const city = parts[3] || '';
            const isp = parts[4] || '';
            let raw = region;

            const _t1 = '\u53f0\u6e7e\u7701';
            const _t2 = '\u53f0\u6e7e';
            const _c1 = '\u4e2d\u56fd';
            const _e1 = '\x43\x4e';
            const _e2 = '\x54\x57\x4e';

            if (province === _t1 || raw.includes(_t1)) {
                country = _t2;
                province = _t2;

                raw = raw.replace(_c1, _t2).replace(_t1, _t2);

                if (raw.endsWith(_e1)) {
                    raw = raw.slice(0, -2) + _e2;
                }
            }

            return {
                raw,
                country,
                province,
                city,
                isp,
            };
        } catch (error) {
            console.error(`IP 查询失败 [${ip}]:`, error);
            return null;
        }
    }
}

export const ipLocationService = new IpLocationService();
