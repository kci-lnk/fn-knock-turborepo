import type {
  WOLDiscoveryPollEvent,
  WOLDiscoveryProgress,
  WOLDiscoveryResult,
  WOLLocalRelay,
  WOLLocalRelayInput,
  WOLRelayInput,
  WOLTarget,
  WOLTargetInput,
} from "@/lib/api";

export const createWolRelayInput = (): WOLRelayInput => ({
  name: "",
  address: "",
  port: 40009,
  enabled: true,
});

export const createWolTargetInput = (name = ""): WOLTargetInput => ({
  name,
  mac: "",
  relayId: null,
  broadcastAddress: null,
  ipAddress: null,
  enabled: true,
  integrations: undefined,
});

export const createWolLocalRelayInput = (): WOLLocalRelayInput => ({
  enabled: false,
  relayId: "",
  keyVersion: 1,
  listenAddress: "0.0.0.0",
  port: 40009,
  broadcastDestinations: ["255.255.255.255:9"],
  allowedSources: [],
  psk: "",
});

export const wolLocalRelayToInput = (
  result: WOLLocalRelay,
): WOLLocalRelayInput => ({
  enabled: result.config.enabled,
  relayId: result.config.relayId,
  keyVersion: result.config.keyVersion,
  listenAddress: result.config.listenAddress,
  port: result.config.port,
  broadcastDestinations: [...result.config.broadcastDestinations],
  allowedSources: [...result.config.allowedSources],
  psk: "",
});

export const wolTargetToEditInput = (target: WOLTarget): WOLTargetInput => ({
  name: target.name,
  mac: target.mac,
  relayId: target.relayId,
  broadcastAddress: target.broadcastAddress,
  ipAddress: target.ipAddress,
  enabled: target.enabled,
  integrations: {
    blinker: {
      enabled: target.integrations.blinker.enabled,
      deviceKey: "",
      bindComponent: target.integrations.blinker.bindComponent,
      skipTlsVerify: true,
    },
    bemfa: {
      // Legacy development builds could enable both providers. Prefer Blinker
      // so the next save converges to the one-provider invariant.
      enabled:
        !target.integrations.blinker.enabled &&
        target.integrations.bemfa.enabled,
      privateKey: "",
      topic: target.integrations.bemfa.topic,
      skipTlsVerify: true,
    },
  },
});

export const updatePendingIds = (
  current: ReadonlySet<string>,
  id: string,
  pending: boolean,
) => {
  const next = new Set(current);
  if (pending) next.add(id);
  else next.delete(id);
  return next;
};

export interface WolDiscoveryViewState {
  progress: WOLDiscoveryProgress | null;
  result: WOLDiscoveryResult | null;
}

export const reduceWolDiscoveryEvent = (
  state: WolDiscoveryViewState,
  event: WOLDiscoveryPollEvent,
): WolDiscoveryViewState => {
  if (event.type === "meta") {
    return {
      progress: event.data.progress,
      result: {
        devices: [],
        networks: event.data.networks,
        durationMs: 0,
        method: "icmp-neighbor",
      },
    };
  }
  if (event.type === "progress") {
    return { ...state, progress: event.data };
  }
  if (event.type === "device") {
    if (!state.result) return state;
    const devices = state.result.devices.filter(
      (device) => device.mac !== event.data.mac,
    );
    devices.push(event.data);
    devices.sort((left, right) =>
      left.ip.localeCompare(right.ip, undefined, { numeric: true }),
    );
    return { ...state, result: { ...state.result, devices } };
  }
  if (event.type === "done") {
    return { ...state, result: event.data };
  }
  return state;
};
