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
  mappings.map((mapping) => ({
    ...mapping,
    basic_auth: { ...mapping.basic_auth },
  }));

const createDisabledHostBasicAuth = (): HostMapping["basic_auth"] => ({
  enabled: false,
  username: "",
  password: "",
});

const normalizeComparableBasicAuth = (
  value: HostMapping["basic_auth"],
): HostMapping["basic_auth"] => {
  const username = value.username.trim();
  const password = value.password;
  if (
    value.enabled !== true ||
    !username ||
    !password ||
    username.includes(":")
  ) {
    return createDisabledHostBasicAuth();
  }

  return {
    enabled: true,
    username,
    password,
  };
};

const hasUsableBasicAuth = (value: HostMapping["basic_auth"]): boolean =>
  normalizeComparableBasicAuth(value).enabled;

const basicAuthMatches = (
  left: HostMapping["basic_auth"],
  right: HostMapping["basic_auth"],
): boolean => {
  const normalizedLeft = normalizeComparableBasicAuth(left);
  const normalizedRight = normalizeComparableBasicAuth(right);
  return (
    normalizedLeft.enabled === normalizedRight.enabled &&
    normalizedLeft.username === normalizedRight.username &&
    normalizedLeft.password === normalizedRight.password
  );
};

const hostKey = (value: string): string => value.trim().toLowerCase();

const targetMatches = (left: string, right: string): boolean =>
  left.trim() === right.trim();

type HostMappingMetadataRefreshItem = {
  mapping: HostMapping;
  refreshTitle: boolean;
  refreshFavicon: boolean;
};

type QueuedHostMappingsMetadataRefresh = {
  mappings: HostMapping[];
  previousMappings: HostMapping[] | null;
};

const toPreviousMappingsByHost = (
  previousMappings: HostMapping[] | null,
): Map<string, HostMapping> | null =>
  previousMappings
    ? new Map(
        previousMappings.map((mapping) => [hostKey(mapping.host), mapping]),
      )
    : null;

const resolveMetadataRefreshDecision = (
  mapping: HostMapping,
  previousByHost: Map<string, HostMapping> | null,
): { refreshTitle: boolean; refreshFavicon: boolean } => {
  const target = mapping.target.trim();
  if (!target) {
    return {
      refreshTitle: false,
      refreshFavicon: false,
    };
  }

  const previous = previousByHost?.get(hostKey(mapping.host));
  const targetChanged =
    previousByHost !== null &&
    (!previous || !targetMatches(previous.target, mapping.target));
  const basicAuthChanged =
    previousByHost !== null &&
    hasUsableBasicAuth(mapping.basic_auth) &&
    (!previous || !basicAuthMatches(previous.basic_auth, mapping.basic_auth));

  return {
    refreshTitle: targetChanged || basicAuthChanged || !mapping.title.trim(),
    refreshFavicon:
      targetChanged || basicAuthChanged || !mapping.favicon.trim(),
  };
};

const enrichHostMappingsMetadata = async (
  mappings: HostMapping[],
  previousByHost: Map<string, HostMapping> | null,
): Promise<{
  items: HostMappingMetadataRefreshItem[];
  summary: HostMappingMetadataRefreshSummary;
}> => {
  const summary = cloneSummary();

  const items = await Promise.all(
    mappings.map(async (mapping) => {
      const decision = resolveMetadataRefreshDecision(mapping, previousByHost);
      if (!decision.refreshTitle && !decision.refreshFavicon) {
        summary.skipped += 1;
        return null;
      }

      const metadata = await fetchUrlMetadata(mapping.target, {
        basicAuth: mapping.basic_auth,
      });
      if (!metadata.ok) {
        summary.failed += 1;
        return null;
      }

      summary.updated += 1;

      return {
        mapping: {
          ...mapping,
          title: decision.refreshTitle ? metadata.data.title : mapping.title,
          favicon: decision.refreshFavicon
            ? metadata.data.favicon
            : mapping.favicon,
        },
        refreshTitle: decision.refreshTitle,
        refreshFavicon: decision.refreshFavicon,
      };
    }),
  );

  return {
    items: items.filter((item): item is HostMappingMetadataRefreshItem =>
      Boolean(item),
    ),
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
  const { items, summary } = await enrichHostMappingsMetadata(
    mappings,
    toPreviousMappingsByHost(previousMappings),
  );
  const refreshedByHost = new Map(
    items.map((item) => [hostKey(item.mapping.host), item.mapping]),
  );

  return {
    mappings: mappings.map(
      (mapping) => refreshedByHost.get(hostKey(mapping.host)) ?? mapping,
    ),
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

      const metadata = await fetchUrlMetadata(mapping.target, {
        basicAuth: mapping.basic_auth,
      });
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

let queuedHostMappingsMetadataRefresh: QueuedHostMappingsMetadataRefresh | null =
  null;
let hostMappingsMetadataRefreshPromise: Promise<void> | null = null;

const mergeMetadataIntoCurrentMappings = (
  currentMappings: HostMapping[],
  refreshedItems: HostMappingMetadataRefreshItem[],
): {
  changed: boolean;
  mappings: HostMapping[];
} => {
  const refreshedByHost = new Map(
    refreshedItems.map((item) => [hostKey(item.mapping.host), item]),
  );
  let changed = false;

  const nextMappings = currentMappings.map((mapping) => {
    const refreshed = refreshedByHost.get(hostKey(mapping.host));
    if (
      !refreshed ||
      !targetMatches(mapping.target, refreshed.mapping.target) ||
      !basicAuthMatches(mapping.basic_auth, refreshed.mapping.basic_auth)
    ) {
      return mapping;
    }

    const nextTitle = refreshed.refreshTitle
      ? refreshed.mapping.title.trim()
      : mapping.title.trim();
    const nextFavicon = refreshed.refreshFavicon
      ? refreshed.mapping.favicon.trim()
      : mapping.favicon.trim();
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

      const { items, summary } = await enrichHostMappingsMetadata(
        snapshot.mappings,
        toPreviousMappingsByHost(snapshot.previousMappings),
      );
      if (summary.updated === 0) {
        continue;
      }

      const currentConfig = await configManager.getConfig();
      const merged = mergeMetadataIntoCurrentMappings(
        currentConfig.host_mappings,
        items,
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
  previousMappings?: HostMapping[],
): void => {
  queuedHostMappingsMetadataRefresh = {
    mappings: cloneHostMappings(mappings),
    previousMappings: previousMappings
      ? cloneHostMappings(previousMappings)
      : null,
  };
  ensureHostMappingsMetadataRefreshWorker();
};
