import type { HostMapping, HostMappingBasicAuth } from "../../types";
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
>;

export const toHostMappingBasicAuthPayload = (
  basicAuth: HostMappingBasicAuth,
): HostMappingBasicAuth => ({
  enabled: basicAuth.enabled,
  username: basicAuth.username.trim(),
  password: basicAuth.password,
});

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
