import {
  isValidCIDR,
  normalizeCidrLines,
} from "../../../../packages/admin-shared/src/utils/cidr";
import { goBackend } from "./go-backend";
import { cidrService } from "./cidr";
import {
  configManager,
  type GatewayVisibilityConfig,
  type GatewayVisibilityRuntimeState,
  type GatewayVisibilitySelection,
} from "./redis";
import { tDefault } from "./i18n";

const gatewayVisibilityT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.gatewayVisibility.${key}`, params);

export interface GatewayVisibilitySelectionInput {
  province: string;
  query_city?: string | null;
}

export interface GatewayVisibilitySummary {
  enabled: boolean;
  selection_count: number;
  custom_cidr_count: number;
  cidr_count: number;
  updated_at: string | null;
}

export interface GatewayVisibilityDetails {
  config: GatewayVisibilityConfig;
  summary: GatewayVisibilitySummary;
}

const buildSelectionKey = (
  province: string,
  queryCity?: string | null,
): string => {
  const normalizedProvince = province.trim();
  const normalizedCity = String(queryCity ?? "").trim();
  return `${normalizedProvince}::${normalizedCity}`;
};

const dedupeSelectionInputs = (
  value: GatewayVisibilitySelectionInput[],
): GatewayVisibilitySelectionInput[] => {
  const result: GatewayVisibilitySelectionInput[] = [];
  const seen = new Set<string>();

  for (const item of value) {
    const province = item.province.trim();
    const queryCity = String(item.query_city ?? "").trim() || null;
    if (!province) continue;

    const key = buildSelectionKey(province, queryCity);
    if (seen.has(key)) continue;
    seen.add(key);

    result.push({
      province,
      query_city: queryCity,
    });
  }

  return result;
};

const toStoredSelection = (
  value: Awaited<ReturnType<typeof cidrService.getCidrs>>["selection"],
): GatewayVisibilitySelection => ({
  province: value.province,
  city: value.city,
  label: value.label,
  value: value.value,
  query_city: value.queryCity,
  is_province_wide: value.isProvinceWide,
  is_municipality: value.isMunicipality,
});

const validateCustomCidrs = (value: string[]): string[] => {
  const normalized = normalizeCidrLines(value);
  const invalid = normalized.filter((cidr) => !isValidCIDR(cidr));

  if (invalid.length > 0) {
    throw new Error(
      gatewayVisibilityT("customCidrInvalid", { cidrs: invalid.join(", ") }),
    );
  }

  return normalized;
};

export const buildGatewayVisibilitySummary = (
  config: GatewayVisibilityConfig,
  runtime: GatewayVisibilityRuntimeState,
): GatewayVisibilitySummary => ({
  enabled: config.enabled,
  selection_count: config.selections.length,
  custom_cidr_count: config.custom_cidrs.length,
  cidr_count: runtime.cidrs.length,
  updated_at: runtime.updated_at,
});

export const getGatewayVisibilityDetails =
  async (): Promise<GatewayVisibilityDetails> => {
    const [config, runtime] = await Promise.all([
      configManager.getGatewayVisibilityConfig(),
      configManager.getGatewayVisibilityRuntimeState(),
    ]);

    return {
      config,
      summary: buildGatewayVisibilitySummary(config, runtime),
    };
  };

export const compileGatewayVisibilityConfig = async (input: {
  enabled: boolean;
  selections: GatewayVisibilitySelectionInput[];
  custom_cidrs: string[];
}): Promise<{
  config: GatewayVisibilityConfig;
  runtime: GatewayVisibilityRuntimeState;
}> => {
  const dedupedSelections = dedupeSelectionInputs(input.selections);
  const customCidrs = validateCustomCidrs(input.custom_cidrs);
  const storedSelections: GatewayVisibilitySelection[] = [];
  const resolvedCidrs: string[] = [];

  for (const selection of dedupedSelections) {
    const lookup = await cidrService.getCidrs({
      province: selection.province,
      city: selection.query_city,
    });

    storedSelections.push(toStoredSelection(lookup.selection));
    resolvedCidrs.push(...lookup.cidrGroups.ipv4, ...lookup.cidrGroups.ipv6);
  }

  const mergedCidrs = normalizeCidrLines([...resolvedCidrs, ...customCidrs]);
  if (input.enabled && mergedCidrs.length === 0) {
    throw new Error(gatewayVisibilityT("emptyEnabledConfig"));
  }

  const now = new Date().toISOString();

  return {
    config: {
      enabled: input.enabled,
      selections: storedSelections,
      custom_cidrs: customCidrs,
    },
    runtime: {
      enabled: input.enabled,
      cidrs: input.enabled ? mergedCidrs : [],
      updated_at: now,
    },
  };
};

export const syncGatewayVisibilityToGateway = async (
  runtime?: GatewayVisibilityRuntimeState | null,
): Promise<GatewayVisibilityRuntimeState> => {
  const nextRuntime =
    runtime ?? (await configManager.getGatewayVisibilityRuntimeState());
  const response = await goBackend.setGatewayVisibility(nextRuntime);

  if (!response.success) {
    throw new Error(response.message || gatewayVisibilityT("syncFailed"));
  }

  return nextRuntime;
};
