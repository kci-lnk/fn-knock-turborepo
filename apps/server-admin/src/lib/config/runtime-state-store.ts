import {
  normalizeGatewayHostResponseRuntimeState,
  normalizeGatewayProxyHeadersRuntimeState,
  normalizeGatewayVisibilityRuntimeState,
  normalizeReverseProxyTrustedIPRuntimeState,
  normalizeSmartConnectRuntimeState,
} from "./app-config";
import {
  DEFAULT_GATEWAY_HOST_RESPONSE_RUNTIME_STATE,
  DEFAULT_GATEWAY_PROXY_HEADERS_RUNTIME_STATE,
  DEFAULT_GATEWAY_VISIBILITY_RUNTIME_STATE,
  DEFAULT_REVERSE_PROXY_TRUSTED_IP_RUNTIME_STATE,
  DEFAULT_SMART_CONNECT_RUNTIME_STATE,
} from "./defaults";
import type { ConfigSectionStore } from "./section-store";
import type {
  GatewayHostResponseRuntimeState,
  GatewayProxyHeadersRuntimeState,
  GatewayVisibilityRuntimeState,
  ReverseProxyTrustedIPRuntimeState,
  SmartConnectRuntimeState,
} from "./types";

export class ConfigRuntimeStateStore {
  private readonly gatewayVisibilityRuntimeKey =
    "fn_knock:gateway:visibility:runtime";
  private readonly gatewayProxyHeadersRuntimeKey =
    "fn_knock:gateway:proxy-headers:runtime";
  private readonly gatewayHostResponseRuntimeKey =
    "fn_knock:gateway:host-response:runtime";
  private readonly reverseProxyTrustedIPsRuntimeKey =
    "fn_knock:reverse-proxy:trusted-ips:runtime";
  private readonly smartConnectRuntimeKey = "fn_knock:smart-connect:runtime";

  constructor(private readonly sections: ConfigSectionStore) {}

  async getGatewayVisibilityRuntimeState(): Promise<GatewayVisibilityRuntimeState> {
    return this.sections.readJson(
      this.gatewayVisibilityRuntimeKey,
      normalizeGatewayVisibilityRuntimeState,
      () => ({
        ...DEFAULT_GATEWAY_VISIBILITY_RUNTIME_STATE,
        cidrs: [],
      }),
      "Failed to parse gateway visibility runtime state",
    );
  }

  async getGatewayProxyHeadersRuntimeState(): Promise<GatewayProxyHeadersRuntimeState> {
    return this.sections.readJson(
      this.gatewayProxyHeadersRuntimeKey,
      normalizeGatewayProxyHeadersRuntimeState,
      () => ({
        ...DEFAULT_GATEWAY_PROXY_HEADERS_RUNTIME_STATE,
        omit_targets: [],
      }),
      "Failed to parse gateway proxy headers runtime state",
    );
  }

  async getGatewayHostResponseRuntimeState(): Promise<GatewayHostResponseRuntimeState> {
    return this.sections.readJson(
      this.gatewayHostResponseRuntimeKey,
      normalizeGatewayHostResponseRuntimeState,
      () => ({
        ...DEFAULT_GATEWAY_HOST_RESPONSE_RUNTIME_STATE,
        omit_targets: [],
      }),
      "Failed to parse gateway host response runtime state",
    );
  }

  async getReverseProxyTrustedIPsRuntimeState(): Promise<ReverseProxyTrustedIPRuntimeState> {
    return this.sections.readJson(
      this.reverseProxyTrustedIPsRuntimeKey,
      normalizeReverseProxyTrustedIPRuntimeState,
      () => ({
        ...DEFAULT_REVERSE_PROXY_TRUSTED_IP_RUNTIME_STATE,
        items: [],
        cidrs: [],
      }),
      "Failed to parse reverse proxy trusted IP runtime state",
    );
  }

  async getSmartConnectRuntimeState(): Promise<SmartConnectRuntimeState> {
    return this.sections.readJson(
      this.smartConnectRuntimeKey,
      normalizeSmartConnectRuntimeState,
      () => ({
        ...DEFAULT_SMART_CONNECT_RUNTIME_STATE,
        synced_domains: [],
      }),
      "Failed to parse smart connect runtime state",
    );
  }

  async saveSmartConnectRuntimeState(
    nextValue: SmartConnectRuntimeState,
  ): Promise<SmartConnectRuntimeState> {
    return this.sections.saveJson(
      this.smartConnectRuntimeKey,
      nextValue,
      normalizeSmartConnectRuntimeState,
    );
  }

  async saveGatewayVisibilityRuntimeState(
    nextValue: GatewayVisibilityRuntimeState,
  ): Promise<GatewayVisibilityRuntimeState> {
    return this.sections.saveJson(
      this.gatewayVisibilityRuntimeKey,
      nextValue,
      normalizeGatewayVisibilityRuntimeState,
    );
  }

  async saveGatewayProxyHeadersRuntimeState(
    nextValue: GatewayProxyHeadersRuntimeState,
  ): Promise<GatewayProxyHeadersRuntimeState> {
    return this.sections.saveJson(
      this.gatewayProxyHeadersRuntimeKey,
      nextValue,
      normalizeGatewayProxyHeadersRuntimeState,
    );
  }

  async saveGatewayHostResponseRuntimeState(
    nextValue: GatewayHostResponseRuntimeState,
  ): Promise<GatewayHostResponseRuntimeState> {
    return this.sections.saveJson(
      this.gatewayHostResponseRuntimeKey,
      nextValue,
      normalizeGatewayHostResponseRuntimeState,
    );
  }

  async saveReverseProxyTrustedIPsRuntimeState(
    nextValue: ReverseProxyTrustedIPRuntimeState,
  ): Promise<ReverseProxyTrustedIPRuntimeState> {
    return this.sections.saveJson(
      this.reverseProxyTrustedIPsRuntimeKey,
      nextValue,
      normalizeReverseProxyTrustedIPRuntimeState,
    );
  }
}
