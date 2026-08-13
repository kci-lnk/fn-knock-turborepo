import type { DDNSNetworkInterfacePayload } from "@/lib/api/ddns";
import type { DDNSIpSource, DDNSUpdateScope } from "./model";

export type DDNSAddressOption = {
  label: string;
  value: string;
};

export type DDNSAddressSourceFieldsProps = {
  configuredNetworkInterface: string;
  configuredNetworkInterfaceLabel: string;
  formatOptionLabel: (option: { labelKey: string }) => string;
  interfaceIPv4Options: DDNSAddressOption[];
  interfaceIPv6Options: DDNSAddressOption[];
  lastIp?: { ipv4: string | null; ipv6: string | null };
  selectionAnchor?: { ipv4: string | null; ipv6: string | null };
  isIpSourceOptionDisabled: (
    providerName: string,
    option: DDNSIpSource,
  ) => boolean;
  isUpdateScopeOptionDisabled: (
    providerName: string,
    option: DDNSUpdateScope,
  ) => boolean;
  providerConfig: Record<string, string>;
  resolvedNetworkInterfaces: DDNSNetworkInterfacePayload[];
  selectedNetworkInterfaceDetail: string;
  selectedProvider: string;
  setFieldValue: (key: string, value: string) => void;
  showInterfaceAddressBlock: boolean;
  showInterfaceIPv4Select: boolean;
  showInterfaceIPv6Select: boolean;
  showSourceDomainBlock: boolean;
  showStaticIPv4Input: boolean;
  showStaticIPv6Input: boolean;
  updateNetworkInterface: (value: string) => void;
  updateIpSource: (value: string) => void;
};
