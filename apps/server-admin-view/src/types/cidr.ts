import type { components as ApiContractComponents } from "@fn-knock/api-contract";

type CidrSchemas = ApiContractComponents["schemas"];

export const CIDR_PROVINCE_WIDE_VALUE = "__province_all__";

export const CIDR_OPERATORS = ["电信", "联通", "移动"] as const;
export type CidrOperator = (typeof CIDR_OPERATORS)[number];

export const getCidrRegionSelectionKey = (selection: {
  province: string;
  query_city?: string | null;
  operator?: CidrOperator | null;
}) =>
  `${selection.province}::${selection.query_city ?? ""}::${selection.operator ?? ""}`;

export const getCidrRegionSelectionLabel = (
  selection: {
    province: string;
    city?: string | null;
    label?: string | null;
    query_city?: string | null;
    operator?: CidrOperator | null;
  },
  options: { includeProvince?: boolean } = {},
) => {
  const province = selection.province.trim();
  const city = selection.city?.trim() || selection.query_city?.trim();
  const label =
    (options.includeProvince && city ? `${province} / ${city}` : "") ||
    selection.label?.trim() ||
    city ||
    province;
  const suffix = selection.operator ? ` · ${selection.operator}` : "";
  return suffix && !label.endsWith(suffix) ? `${label}${suffix}` : label;
};

export type CidrCapabilitiesPayload = CidrSchemas["CidrCapabilitiesData"];
export type CidrProvinceItem = CidrSchemas["CidrProvinceItemData"];
export type CidrProvinceOption = CidrSchemas["CidrProvinceOptionData"];
export type CidrProvincesPayload = CidrSchemas["CidrProvincesData"];
export type CidrCityItem = CidrSchemas["CidrCityItemData"];
export type CidrCityOption = CidrSchemas["CidrCityOptionData"];
export type CidrCitiesPayload = CidrSchemas["CidrCitiesData"];
export type CidrSelectorPayload = CidrSchemas["CidrSelectorData"];
export type CidrSelectionPayload = CidrSchemas["CidrSelectionData"];
export type CidrLookupPayload = CidrSchemas["CidrLookupData"];
