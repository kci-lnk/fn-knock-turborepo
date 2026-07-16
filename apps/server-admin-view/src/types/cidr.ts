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

export interface CidrCapabilitiesPayload {
  source: "online" | "custom";
  operatorFiltering: {
    supported: boolean;
    operators: CidrOperator[];
    minimumContainerVersion: string;
  };
}

export interface CidrProvinceItem {
  name: string;
  cityCount: number;
  isMunicipality: boolean;
  hasChildren: boolean;
}

export interface CidrProvinceOption {
  label: string;
  value: string;
  cityCount: number;
  isMunicipality: boolean;
}

export interface CidrProvincesPayload {
  items: CidrProvinceItem[];
  options: CidrProvinceOption[];
  total: number;
}

export interface CidrCityItem {
  name: string;
  ipv4Count: number;
  ipv6Count: number;
}

export interface CidrCityOption {
  label: string;
  value: string;
  queryCity: string | null;
  isProvinceWide: boolean;
  isMunicipality: boolean;
  ipv4Count: number;
  ipv6Count: number;
}

export interface CidrCitiesPayload {
  province: string;
  items: CidrCityItem[];
  options: CidrCityOption[];
  total: number;
  isMunicipality: boolean;
  supportsProvinceWide: boolean;
  defaultValue: string;
}

export interface CidrSelectorPayload {
  provinces: CidrProvincesPayload;
  cities: CidrCitiesPayload | null;
}

export interface CidrSelectionPayload {
  province: string;
  city: string | null;
  label: string;
  value: string;
  queryCity: string | null;
  operator: CidrOperator | null;
  isProvinceWide: boolean;
  isMunicipality: boolean;
}

export interface CidrLookupPayload {
  province: string;
  city: string | null;
  selection: CidrSelectionPayload;
  cidrGroups: {
    ipv4: string[];
    ipv6: string[];
  };
  counts: {
    ipv4: number;
    ipv6: number;
  };
  totalCount: number;
}
