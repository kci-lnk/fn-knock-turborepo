export const CIDR_PROVINCE_WIDE_VALUE = "__province_all__";

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
