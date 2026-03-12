import { redis } from "./redis";

export type TunnelType = "frp" | "cloudflared";

export type TunnelRuntimeState = {
  frp_enabled: boolean;
  cloudflared_enabled: boolean;
  last_tunnel: TunnelType;
  updated_at: string;
};

const KEY = "fn_knock:tunnel:runtime";

const DEFAULT_STATE: TunnelRuntimeState = {
  frp_enabled: false,
  cloudflared_enabled: false,
  last_tunnel: "frp",
  updated_at: new Date(0).toISOString(),
};

const isTunnelType = (value: unknown): value is TunnelType =>
  value === "frp" || value === "cloudflared";

const sanitizeState = (raw: unknown): TunnelRuntimeState => {
  if (!raw || typeof raw !== "object") return { ...DEFAULT_STATE };
  const obj = raw as Record<string, unknown>;
  const updated_at = typeof obj.updated_at === "string" ? obj.updated_at : new Date().toISOString();

  // Backward compatibility: old shape { tunnel, enabled }
  if (!("frp_enabled" in obj) && !("cloudflared_enabled" in obj) && "tunnel" in obj && "enabled" in obj) {
    const tunnel = isTunnelType(obj.tunnel) ? obj.tunnel : DEFAULT_STATE.last_tunnel;
    const enabled = typeof obj.enabled === "boolean" ? obj.enabled : false;
    return {
      frp_enabled: tunnel === "frp" ? enabled : false,
      cloudflared_enabled: tunnel === "cloudflared" ? enabled : false,
      last_tunnel: tunnel,
      updated_at,
    };
  }

  const frp_enabled = typeof obj.frp_enabled === "boolean" ? obj.frp_enabled : DEFAULT_STATE.frp_enabled;
  const cloudflared_enabled =
    typeof obj.cloudflared_enabled === "boolean" ? obj.cloudflared_enabled : DEFAULT_STATE.cloudflared_enabled;
  const last_tunnel = isTunnelType(obj.last_tunnel) ? obj.last_tunnel : DEFAULT_STATE.last_tunnel;
  return { frp_enabled, cloudflared_enabled, last_tunnel, updated_at };
};

const writeState = async (state: TunnelRuntimeState): Promise<TunnelRuntimeState> => {
  await redis.set(KEY, JSON.stringify(state));
  return state;
};

export async function getTunnelRuntimeState(): Promise<TunnelRuntimeState> {
  const raw = await redis.get(KEY);
  if (!raw) return { ...DEFAULT_STATE };
  try {
    return sanitizeState(JSON.parse(raw));
  } catch {
    return { ...DEFAULT_STATE };
  }
}

export async function markTunnelRunning(tunnel: TunnelType): Promise<TunnelRuntimeState> {
  const current = await getTunnelRuntimeState();
  const state: TunnelRuntimeState = {
    ...current,
    frp_enabled: tunnel === "frp" ? true : current.frp_enabled,
    cloudflared_enabled: tunnel === "cloudflared" ? true : current.cloudflared_enabled,
    last_tunnel: tunnel,
    updated_at: new Date().toISOString(),
  };
  return writeState(state);
}

export async function markTunnelStopped(tunnel: TunnelType): Promise<TunnelRuntimeState> {
  const current = await getTunnelRuntimeState();
  if (tunnel === "frp" && !current.frp_enabled) {
    return current;
  }
  if (tunnel === "cloudflared" && !current.cloudflared_enabled) {
    return current;
  }
  const next: TunnelRuntimeState = {
    ...current,
    frp_enabled: tunnel === "frp" ? false : current.frp_enabled,
    cloudflared_enabled: tunnel === "cloudflared" ? false : current.cloudflared_enabled,
    updated_at: new Date().toISOString(),
  };
  return writeState(next);
}

export async function shouldResumeTunnel(tunnel: TunnelType): Promise<boolean> {
  const state = await getTunnelRuntimeState();
  return tunnel === "frp" ? state.frp_enabled : state.cloudflared_enabled;
}
