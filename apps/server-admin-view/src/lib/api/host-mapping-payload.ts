import type {
  GatewayVisibilitySelection,
  HostMapping,
  HostMappingBasicAuth,
} from "../../types";
import { normalizeHostMappingAvailability } from "../host-mapping-availability";

type HostMappingUpdatePayload = Pick<
  HostMapping,
  | "host"
  | "target"
  | "waf_enabled"
  | "use_auth"
  | "access_mode"
  | "suppress_toolbar"
  | "preserve_host"
  | "is_default"
  | "disabled"
  | "availability"
  | "protocol_mode"
  | "basic_auth"
  | "locations"
  | "title_override"
> & {
  visibility: {
    mode: HostMapping["visibility"]["mode"];
    selections: Pick<GatewayVisibilitySelection, "province" | "query_city">[];
    custom_cidrs: string[];
  };
};

export const toHostMappingBasicAuthPayload = (
  basicAuth?: Partial<HostMappingBasicAuth> | null,
): HostMappingBasicAuth => {
  const username =
    typeof basicAuth?.username === "string" ? basicAuth.username.trim() : "";
  const password =
    typeof basicAuth?.password === "string" ? basicAuth.password : "";

  if (
    basicAuth?.enabled !== true ||
    !username ||
    !password ||
    username.includes(":")
  ) {
    return {
      enabled: false,
      username: "",
      password: "",
    };
  }

  return {
    enabled: true,
    username,
    password,
  };
};

export const toHostMappingUpdatePayload = (
  mapping: HostMapping,
): HostMappingUpdatePayload => ({
  host: mapping.host,
  target: mapping.target,
  waf_enabled: mapping.waf_enabled !== false,
  use_auth: mapping.use_auth,
  access_mode: mapping.access_mode,
  suppress_toolbar: mapping.suppress_toolbar,
  preserve_host: mapping.preserve_host,
  is_default: mapping.is_default === true,
  disabled: mapping.disabled === true,
  availability: normalizeHostMappingAvailability(mapping.availability),
  visibility: {
    mode:
      mapping.visibility?.mode === "custom" ||
      mapping.visibility?.mode === "disabled"
        ? mapping.visibility.mode
        : "inherit",
    selections: (mapping.visibility?.selections ?? []).map((selection) => ({
      province: selection.province,
      query_city: selection.query_city,
    })),
    custom_cidrs: [...(mapping.visibility?.custom_cidrs ?? [])],
  },
  protocol_mode:
    mapping.protocol_mode === "http1" || mapping.protocol_mode === "http2"
      ? mapping.protocol_mode
      : "auto",
  basic_auth: toHostMappingBasicAuthPayload(mapping.basic_auth),
  locations: (mapping.locations ?? []).map((location) => ({
    path: location.path.trim(),
    match: location.match,
    action: location.action,
    target: location.target.trim(),
    strip_path: location.strip_path,
    rewrite_html: location.rewrite_html,
    response: {
      status: location.response.status,
      content_type: location.response.content_type.trim(),
      headers: { ...location.response.headers },
      body: location.response.body,
    },
  })),
  title_override: mapping.title_override.trim(),
});
