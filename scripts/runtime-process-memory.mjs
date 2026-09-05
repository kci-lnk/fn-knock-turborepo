import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export const parseLinuxProcessMemory = (status) => {
  const kilobytes = (field) => {
    const value = status.match(new RegExp(`^${field}:\\s+(\\d+)\\s+kB$`, "m"));
    return value ? Number(value[1]) * 1024 : null;
  };
  return {
    rss_bytes: kilobytes("VmRSS"),
    peak_rss_bytes: kilobytes("VmHWM"),
  };
};

// Read the owned process directly. The application's health endpoint caches
// its RSS for five seconds and cannot observe short allocation bursts.
export const readProcessMemory = async (pid, signal) => {
  signal?.throwIfAborted();
  if (!Number.isSafeInteger(pid) || pid <= 0) {
    throw new Error("process memory sampling requires a positive PID");
  }
  if (process.platform === "linux") {
    const memory = parseLinuxProcessMemory(
      await readFile(`/proc/${pid}/status`, { encoding: "utf8", signal }),
    );
    if (memory.rss_bytes === null) {
      throw new Error(`process ${pid} has no resident-memory measurement`);
    }
    return memory;
  }
  if (process.platform === "darwin") {
    const { stdout } = await execFileAsync(
      "ps",
      ["-o", "rss=", "-p", String(pid)],
      {
        timeout: 2_000,
        signal,
      },
    );
    const value = stdout.trim();
    if (!/^\d+$/u.test(value)) throw new Error(`process ${pid} has no RSS`);
    return { rss_bytes: Number(value) * 1024, peak_rss_bytes: null };
  }
  // Other platforms retain health-snapshot sampling; do not present it as an
  // independent high-frequency measurement.
  return null;
};
