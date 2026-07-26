import type { RuntimeCapabilities, RuntimeProfile } from "../types";

// This deliberately requires both signals. A stale or manually restored
// capability must not expose host-firewall controls outside the standard FPK.
export const canUseFnosConnectWafForRuntime = (
  profile?: RuntimeProfile,
  capabilities?: RuntimeCapabilities,
): boolean =>
  profile?.deployment_target === "fpk" &&
  capabilities?.fnos_connect_waf_available === true;
