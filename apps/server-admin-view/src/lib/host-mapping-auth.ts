import type { HostMapping } from "@/types";

/** Whether the mapping has any routes governed by its authentication policy. */
export const hostMappingUsesAuth = (
  mapping: Pick<HostMapping, "use_auth" | "locations" | "service_role">,
): boolean =>
  mapping.service_role !== "auth" &&
  (mapping.use_auth ||
    mapping.locations?.some(
      (location) => location.auth_mode === "require_login",
    ) === true);
