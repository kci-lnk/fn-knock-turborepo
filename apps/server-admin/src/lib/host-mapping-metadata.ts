import type { HostMapping } from "./redis";
import { fetchUrlMetadata } from "./url-metadata";

export interface HostMappingMetadataRefreshSummary {
  updated: number;
  failed: number;
  skipped: number;
}

export const resolveHostMappingDisplayTitle = (
  mapping: Pick<HostMapping, "title" | "title_override">,
): string => mapping.title_override.trim() || mapping.title.trim();

const cloneSummary = (): HostMappingMetadataRefreshSummary => ({
  updated: 0,
  failed: 0,
  skipped: 0,
});

export const enrichHostMappingsMetadataOnSave = async (
  mappings: HostMapping[],
  previousMappings: HostMapping[],
): Promise<{
  mappings: HostMapping[];
  summary: HostMappingMetadataRefreshSummary;
}> => {
  const previousByHost = new Map(
    previousMappings.map((item) => [item.host, item]),
  );
  const summary = cloneSummary();

  const nextMappings = await Promise.all(
    mappings.map(async (mapping) => {
      const previous = previousByHost.get(mapping.host);
      const shouldRefreshTitle =
        !previous ||
        previous.target !== mapping.target ||
        !mapping.title.trim();
      const shouldRefreshFavicon =
        !previous ||
        previous.target !== mapping.target ||
        !mapping.favicon.trim();

      if (!shouldRefreshTitle && !shouldRefreshFavicon) {
        summary.skipped += 1;
        return mapping;
      }

      const metadata = await fetchUrlMetadata(mapping.target);
      if (!metadata.ok) {
        summary.failed += 1;
        return mapping;
      }

      summary.updated += 1;

      return {
        ...mapping,
        title: shouldRefreshTitle ? metadata.data.title : mapping.title,
        favicon: shouldRefreshFavicon ? metadata.data.favicon : mapping.favicon,
      };
    }),
  );

  return {
    mappings: nextMappings,
    summary,
  };
};

export const refreshAllHostMappingTitles = async (
  mappings: HostMapping[],
): Promise<{
  mappings: HostMapping[];
  summary: HostMappingMetadataRefreshSummary;
}> => {
  const summary = cloneSummary();

  const nextMappings = await Promise.all(
    mappings.map(async (mapping) => {
      if (!mapping.target.trim()) {
        summary.skipped += 1;
        return mapping;
      }

      const metadata = await fetchUrlMetadata(mapping.target);
      if (!metadata.ok) {
        summary.failed += 1;
        return mapping;
      }

      summary.updated += 1;

      return {
        ...mapping,
        title: metadata.data.title,
      };
    }),
  );

  return {
    mappings: nextMappings,
    summary,
  };
};
