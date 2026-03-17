import { promises as fs } from "node:fs";
import path from "node:path";

export const SSL_CERT_SHARE_NAME = "fn-knock";

const MAX_SHARED_FILES = 500;
const MAX_SHARED_FILE_SIZE = 512 * 1024;
const MAX_SHARED_SCAN_DEPTH = 3;

export interface SharedDataFileEntry {
    name: string;
    relativePath: string;
    extension: string;
    size: number;
    modifiedAt: string;
}

function getConfiguredShareDirectory() {
    const explicit = process.env.FN_KNOCK_ROOT_SHARE_DIR?.trim() || process.env.FN_KNOCK_CERT_SHARE_DIR?.trim();
    if (explicit) {
        return explicit;
    }

    const dataSharePaths = process.env.TRIM_DATA_SHARE_PATHS?.trim();
    if (!dataSharePaths) {
        return "";
    }

    const candidates = dataSharePaths
        .split(":")
        .map((value) => value.trim())
        .filter(Boolean);

    if (!candidates.length) {
        return "";
    }

    return candidates.reduce((selected, current) =>
        current.length < selected.length ? current : selected
    );
}

function getNormalizedExtension(fileName: string) {
    return path.extname(fileName).toLowerCase();
}

function resolveSharePath(rootDir: string, relativePath: string) {
    const sanitized = relativePath.replace(/\\/g, "/").trim();
    const resolved = path.resolve(rootDir, sanitized);
    const relativeToRoot = path.relative(rootDir, resolved);

    if (
        !sanitized ||
        sanitized.startsWith("/") ||
        relativeToRoot.startsWith("..") ||
        path.isAbsolute(relativeToRoot)
    ) {
        throw new Error("非法的共享文件路径");
    }

    return resolved;
}

async function walkFiles(
    currentDir: string,
    rootDir: string,
    bucket: SharedDataFileEntry[],
    depth: number,
): Promise<void> {
    if (bucket.length >= MAX_SHARED_FILES) {
        return;
    }

    const entries = await fs.readdir(currentDir, { withFileTypes: true });

    for (const entry of entries) {
        if (bucket.length >= MAX_SHARED_FILES) {
            return;
        }

        const fullPath = path.join(currentDir, entry.name);
        if (entry.isDirectory()) {
            if (depth >= MAX_SHARED_SCAN_DEPTH) {
                continue;
            }
            await walkFiles(fullPath, rootDir, bucket, depth + 1);
            continue;
        }

        if (!entry.isFile()) {
            continue;
        }

        const stats = await fs.stat(fullPath);
        bucket.push({
            name: entry.name,
            relativePath: path.relative(rootDir, fullPath).split(path.sep).join("/"),
            extension: getNormalizedExtension(entry.name),
            size: stats.size,
            modifiedAt: stats.mtime.toISOString(),
        });
    }
}

export async function listSSLSharedFiles() {
    const directory = getConfiguredShareDirectory();
    if (!directory) {
        return {
            shareName: SSL_CERT_SHARE_NAME,
            available: false,
            files: [] as SharedDataFileEntry[],
        };
    }

    try {
        const stats = await fs.stat(directory);
        if (!stats.isDirectory()) {
            return {
                shareName: SSL_CERT_SHARE_NAME,
                available: false,
                files: [] as SharedDataFileEntry[],
            };
        }
    } catch {
        return {
            shareName: SSL_CERT_SHARE_NAME,
            available: false,
            files: [] as SharedDataFileEntry[],
        };
    }

    const files: SharedDataFileEntry[] = [];
    await walkFiles(directory, directory, files, 0);
    files.sort((left, right) => {
        const timeDiff = new Date(right.modifiedAt).getTime() - new Date(left.modifiedAt).getTime();
        if (timeDiff !== 0) {
            return timeDiff;
        }
        return left.relativePath.localeCompare(right.relativePath, "zh-CN");
    });

    return {
        shareName: SSL_CERT_SHARE_NAME,
        available: true,
        files,
    };
}

export async function readSSLSharedFile(relativePath: string) {
    const directory = getConfiguredShareDirectory();
    if (!directory) {
        throw new Error("未找到飞牛共享目录，请确认应用资源已正确配置");
    }

    const filePath = resolveSharePath(directory, relativePath);
    const stats = await fs.stat(filePath);
    if (!stats.isFile()) {
        throw new Error("只能读取共享目录中的文件");
    }
    if (stats.size > MAX_SHARED_FILE_SIZE) {
        throw new Error("文件过大，请仅放入证书或私钥文本文件");
    }

    const content = await fs.readFile(filePath, "utf8");

    return {
        file: {
            name: path.basename(filePath),
            relativePath,
            extension: getNormalizedExtension(filePath),
            size: stats.size,
            modifiedAt: stats.mtime.toISOString(),
        } satisfies SharedDataFileEntry,
        content: content.replace(/^\uFEFF/, ""),
    };
}
