import { ref, type Ref } from "vue";
import type {
  DDNSHttpTransport,
  DDNSPublicDnsProvider,
  DDNSPublicCheckSourcesPayload,
  DDNSStatusPayload,
  DDNSTargetSummaryPayload,
} from "@/lib/api";
import {
  DEFAULT_DDNS_HTTP_TRANSPORT,
  DEFAULT_DDNS_PUBLIC_DNS_PROVIDER,
  DEFAULT_DDNS_IP_SOURCE,
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
  DEFAULT_DDNS_UPDATE_SCOPE,
  normalizeDDNSHttpTransport,
  normalizeDDNSPublicDnsProvider,
  normalizeIpSource,
  normalizeNetworkInterface,
  normalizePublicCheckSources,
  normalizeUpdateIntervalMinutes,
  normalizeUpdateScope,
  type DDNSIpSource,
  type DDNSUpdateScope,
  type LastCheck,
  type LastIP,
} from "./model";

interface UseDDNSStatusOptions {
  selectedProvider: Ref<string>;
}

export function useDDNSStatus({ selectedProvider }: UseDDNSStatusOptions) {
  const enabled = ref(true);
  const savedProvider = ref("");
  const lastIP = ref<LastIP>({ ipv4: null, ipv6: null, updated_at: null });
  const selectionAnchor = ref<LastIP>({
    ipv4: null,
    ipv6: null,
    updated_at: null,
  });
  const lastCheck = ref<LastCheck>({
    checked_at: null,
    outcome: null,
    message: null,
  });
  const updateIntervalMinutes = ref(DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES);
  const publicCheckSources = ref<DDNSPublicCheckSourcesPayload>(
    normalizePublicCheckSources(undefined),
  );
  const defaultPublicCheckSources = ref<DDNSPublicCheckSourcesPayload>(
    normalizePublicCheckSources(undefined),
  );
  const httpTransport = ref<DDNSHttpTransport>(DEFAULT_DDNS_HTTP_TRANSPORT);
  const publicDnsProvider = ref<DDNSPublicDnsProvider>(
    DEFAULT_DDNS_PUBLIC_DNS_PROVIDER,
  );
  const statusUpdateScope = ref<DDNSUpdateScope>(DEFAULT_DDNS_UPDATE_SCOPE);
  const statusIpSource = ref<DDNSIpSource>(DEFAULT_DDNS_IP_SOURCE);
  const statusNetworkInterface = ref("");
  const targetSummaries = ref<DDNSTargetSummaryPayload[]>([]);

  function applyStatus(
    status: DDNSStatusPayload,
    options: { syncEnabled?: boolean; syncProvider?: boolean } = {},
  ) {
    if (options.syncEnabled !== false) {
      enabled.value = status.enabled;
    }
    savedProvider.value = status.provider || "";
    if (options.syncProvider !== false) {
      selectedProvider.value = savedProvider.value;
    }
    lastIP.value = status.lastIP;
    selectionAnchor.value = status.selectionAnchor ?? status.lastIP;
    lastCheck.value = status.lastCheck;
    updateIntervalMinutes.value = normalizeUpdateIntervalMinutes(
      status.updateIntervalMinutes,
    );
    defaultPublicCheckSources.value = normalizePublicCheckSources(
      status.defaultPublicCheckSources,
    );
    publicCheckSources.value = normalizePublicCheckSources(
      status.publicCheckSources,
      defaultPublicCheckSources.value,
    );
    httpTransport.value = normalizeDDNSHttpTransport(status.httpTransport);
    publicDnsProvider.value = normalizeDDNSPublicDnsProvider(
      status.publicDnsProvider,
    );
    statusUpdateScope.value = normalizeUpdateScope(status.updateScope);
    statusIpSource.value = normalizeIpSource(status.ipSource);
    statusNetworkInterface.value = normalizeNetworkInterface(
      status.networkInterface,
    );
    targetSummaries.value = status.targets || [];
  }

  return {
    applyStatus,
    defaultPublicCheckSources,
    enabled,
    httpTransport,
    publicDnsProvider,
    lastCheck,
    lastIP,
    selectionAnchor,
    publicCheckSources,
    savedProvider,
    statusIpSource,
    statusNetworkInterface,
    statusUpdateScope,
    targetSummaries,
    updateIntervalMinutes,
  };
}
