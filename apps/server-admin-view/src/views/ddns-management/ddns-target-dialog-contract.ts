import type { DDNSNetworkInterfacePayload } from "@/lib/api/ddns";
import type {
  DDNSIpSource,
  DDNSUpdateScope,
  TargetDialogState,
} from "./model";

export type DDNSLabelKeyOption<
  TValue extends DDNSIpSource | DDNSUpdateScope,
> = {
  labelKey: string;
  value: TValue;
};

export type DDNSAddressOption = {
  label: string;
  value: string;
};

export type DDNSTargetAddressFieldsProps = {
  formatOptionLabel: (
    option: DDNSLabelKeyOption<DDNSIpSource | DDNSUpdateScope>,
  ) => string;
  ipv4Options: DDNSAddressOption[];
  ipv6Options: DDNSAddressOption[];
  isIpSourceOptionDisabled: (
    providerName: string,
    option: DDNSIpSource,
  ) => boolean;
  isUpdateScopeOptionDisabled: (
    providerName: string,
    option: DDNSUpdateScope,
  ) => boolean;
  networkInterfaceLabel: string;
  resolvedNetworkInterfaces: DDNSNetworkInterfacePayload[];
  shouldShowDomainBlock: boolean;
  shouldShowInterfaceBlock: boolean;
  shouldShowStaticBlock: boolean;
  state: TargetDialogState;
  updateScope: DDNSUpdateScope;
};
