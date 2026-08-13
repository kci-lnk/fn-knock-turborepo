import type { SelectableGatewayLogEntry } from "./useGatewayLogIpSelection";

export interface GatewayRequestLogRowProps {
  blockIpsFromLogs: (ips: string[]) => Promise<void> | void;
  entry: SelectableGatewayLogEntry;
  getConnectionSourceText: (entry: SelectableGatewayLogEntry) => string;
  getEntryIpLocationText: (entry: SelectableGatewayLogEntry) => string;
  goToWafTrace: (traceId?: string) => void;
  isGeneralBlacklisted: (ip: string) => boolean;
  isMutatingBlacklistIps: boolean;
  isSelected: boolean;
  releaseIpsFromLogs: (ips: string[]) => Promise<void> | void;
  toggleSelection: (key?: string) => void;
  viewDetails: (entry: SelectableGatewayLogEntry) => void;
}
