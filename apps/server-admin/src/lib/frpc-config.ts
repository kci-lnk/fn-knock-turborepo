import fs from "node:fs";
import path from "node:path";
import { dataPath } from "./AppDirManager";

const FRPC_DIR = path.join(dataPath, "frp");
const FRPC_TOML = path.join(FRPC_DIR, "frpc.toml");

const normalizePort = (value: string | undefined | null): number | null => {
  const parsed = Number.parseInt(String(value ?? "").trim(), 10);
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 65535) return null;
  return parsed;
};

export const readFrpcConfig = (): string | null => {
  try {
    if (!fs.existsSync(FRPC_DIR) || !fs.existsSync(FRPC_TOML)) return null;
    return fs.readFileSync(FRPC_TOML, "utf-8");
  } catch {
    return null;
  }
};

export const extractFrpcRemotePort = (content: string): number | null => {
  const next =
    content.match(/^\s*remotePort\s*=\s*(\d+)\s*$/m)?.[1] ||
    content.match(/^\s*remote_port\s*=\s*(\d+)\s*$/m)?.[1] ||
    null;
  return normalizePort(next);
};

export const resolveFrpcRemotePort = (): number | null => {
  const content = readFrpcConfig();
  if (!content) return null;
  return extractFrpcRemotePort(content);
};
