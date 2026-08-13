import type { AppConfig, HostMapping, HostMappingGroup } from "../../types";

export const HOST_MAPPINGS_REVISION_HEADER = "x-host-mappings-revision";
export const HOST_MAPPING_CATALOG_REVISION_HEADER =
  "x-host-mapping-catalog-revision";

export const hostMappingsRevisionFromHeaders = (
  headers: Record<string, unknown>,
): string | null => {
  const value = String(headers[HOST_MAPPINGS_REVISION_HEADER] ?? "").trim();
  return value || null;
};

export interface RevisionedConfig {
  config: AppConfig;
  hostMappingsRevision: string | null;
  hostMappingCatalogRevision: string | null;
}

export interface RevisionedHostMappings {
  mappings: HostMapping[];
  revision: string | null;
}

export interface RevisionedHostMappingCatalog {
  mappings: HostMapping[];
  groups: HostMappingGroup[];
  groupedView: boolean;
  revision: string | null;
  hostMappingsRevision: string | null;
}
