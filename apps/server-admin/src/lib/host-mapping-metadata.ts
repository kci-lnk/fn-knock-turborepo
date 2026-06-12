import { configManager, type HostMapping } from "./redis";
import { syncGatewayPortalHostRulesIfTitleMode } from "./gateway-portal";
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

const cloneHostMappings = (mappings: HostMapping[]): HostMapping[] =>
  mappings.map((mapping) => ({ ...mapping }));

const shouldBackfillHostMappingMetadata = (
  mapping: Pick<HostMapping, "target" | "title" | "favicon">,
): boolean =>
  Boolean(mapping.target.trim()) &&
  (!mapping.title.trim() || !mapping.favicon.trim());

const targetMatches = (left: string, right: string): boolean =>
  left.trim() === right.trim();

const enrichMissingHostMappingsMetadata = async (
  mappings: HostMapping[],
): Promise<{
  mappings: HostMapping[];
  summary: HostMappingMetadataRefreshSummary;
}> => {
  const summary = cloneSummary();

  const nextMappings = await Promise.all(
    mappings.map(async (mapping) => {
      if (!shouldBackfillHostMappingMetadata(mapping)) {
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
        title: mapping.title.trim() || metadata.data.title,
        favicon: mapping.favicon.trim() || metadata.data.favicon,
      };
    }),
  );

  return {
    mappings: nextMappings,
    summary,
  };
};

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
        favicon: metadata.data.favicon,
      };
    }),
  );

  return {
    mappings: nextMappings,
    summary,
  };
};

let queuedHostMappingsMetadataRefresh: HostMapping[] | null = null;
let hostMappingsMetadataRefreshPromise: Promise<void> | null = null;

const mergeMetadataIntoCurrentMappings = (
  currentMappings: HostMapping[],
  refreshedMappings: HostMapping[],
): {
  changed: boolean;
  mappings: HostMapping[];
} => {
  const refreshedByHost = new Map(
    refreshedMappings.map((mapping) => [mapping.host, mapping]),
  );
  let changed = false;

  const nextMappings = currentMappings.map((mapping) => {
    const refreshed = refreshedByHost.get(mapping.host);
    if (!refreshed || !targetMatches(mapping.target, refreshed.target)) {
      return mapping;
    }

    const nextTitle = mapping.title.trim() || refreshed.title.trim();
    const nextFavicon = mapping.favicon.trim() || refreshed.favicon.trim();
    if (
      nextTitle === mapping.title.trim() &&
      nextFavicon === mapping.favicon.trim()
    ) {
      return mapping;
    }

    changed = true;
    return {
      ...mapping,
      title: nextTitle,
      favicon: nextFavicon,
    };
  });

  return {
    changed,
    mappings: nextMappings,
  };
};

const ensureHostMappingsMetadataRefreshWorker = (): void => {
  if (hostMappingsMetadataRefreshPromise) {
    return;
  }

  hostMappingsMetadataRefreshPromise = (async () => {
    while (queuedHostMappingsMetadataRefresh) {
      const snapshot = queuedHostMappingsMetadataRefresh;
      queuedHostMappingsMetadataRefresh = null;

      const { mappings, summary } =
        await enrichMissingHostMappingsMetadata(snapshot);
      if (summary.updated === 0) {
        continue;
      }

      const currentConfig = await configManager.getConfig();
      const merged = mergeMetadataIntoCurrentMappings(
        currentConfig.host_mappings,
        mappings,
      );

      if (!merged.changed) {
        continue;
      }

      await configManager.updateHostMappings(merged.mappings);
      try {
        await syncGatewayPortalHostRulesIfTitleMode({
          run_type: currentConfig.run_type,
          reverse_proxy_submode: currentConfig.reverse_proxy_submode,
          gateway_portal: currentConfig.gateway_portal,
          host_mappings: merged.mappings,
        });
      } catch (error) {
        console.error(
          "[host-mappings] failed to sync refreshed titles to gateway:",
          error,
        );
      }
    }
  })()
    .catch((error) => {
      console.error(
        "[host-mappings] failed to refresh metadata in background:",
        error,
      );
    })
    .finally(() => {
      hostMappingsMetadataRefreshPromise = null;
      if (queuedHostMappingsMetadataRefresh) {
        ensureHostMappingsMetadataRefreshWorker();
      }
    });
};

export const scheduleHostMappingsMetadataRefresh = (
  mappings: HostMapping[],
): void => {
  queuedHostMappingsMetadataRefresh = cloneHostMappings(mappings);
  ensureHostMappingsMetadataRefreshWorker();
};
