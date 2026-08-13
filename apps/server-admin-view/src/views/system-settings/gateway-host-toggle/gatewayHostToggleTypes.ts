export type GatewayHostToggleField = "preserve_host" | "send_proxy_headers";

export type GatewayHostConfigStoreKey =
  | "gateway_host_response"
  | "gateway_proxy_headers";

export type GatewayHostToggleItem = {
  host: string;
  target: string;
  title: string;
} & Record<string, unknown>;

export type GatewayHostToggleDetails = {
  availability: {
    available: boolean;
    reason: string;
  };
  config: {
    disabled_hosts: string[];
  };
  items: GatewayHostToggleItem[];
  summary: {
    disabled_count: number;
    total_count: number;
    updated_at: string | null;
  };
};

export type GatewayHostToggleOptions = {
  configStoreKey: GatewayHostConfigStoreKey;
  fetchDetails: () => Promise<GatewayHostToggleDetails>;
  messageKeyPrefix: string;
  saveDetails: (payload: {
    disabled_hosts: string[];
  }) => Promise<GatewayHostToggleDetails>;
  toggleField: GatewayHostToggleField;
};
