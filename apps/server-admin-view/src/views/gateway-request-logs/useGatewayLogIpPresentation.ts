import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { useGatewayRequestLogsResource } from "./useGatewayRequestLogsResource";

export const useGatewayLogIpPresentation = (
  resource: ReturnType<typeof useGatewayRequestLogsResource>,
) => {
  const {
    clientIp,
    openIp,
    getSnapshot,
    isIpOverview,
    loading,
    loadError,
    ipGroups,
    hasFilters,
    selectedDate,
  } = resource;
  const { t } = useI18n();
  const root = ref<HTMLElement | null>(null);
  let overviewScroll: Array<{
    element: HTMLElement;
    top: number;
    left: number;
  }> = [];
  const handleOpenIp = (ip: string) => {
    overviewScroll = [];
    for (let element = root.value; element; element = element.parentElement) {
      overviewScroll.push({
        element,
        top: element.scrollTop,
        left: element.scrollLeft,
      });
    }
    void openIp(ip);
  };
  watch(clientIp, async (ip, previous) => {
    if (ip && !previous) {
      await nextTick();
      for (const { element } of overviewScroll) element.scrollTop = 0;
    } else if (!ip && previous) {
      await nextTick();
      for (const { element, top, left } of overviewScroll) {
        element.scrollTop = top;
        element.scrollLeft = left;
      }
    }
  });
  const ipLocation = (ip: string) => {
    const snapshot = getSnapshot(ip);
    if (snapshot?.location) return snapshot.location;
    if (snapshot?.status === "queued" || snapshot?.status === "processing")
      return t("admin.hostActiveIps.resolving");
    if (snapshot?.status === "failed")
      return t("admin.hostActiveIps.unavailable");
    return "";
  };
  const ipSummary = computed(() => {
    if (!isIpOverview.value) return undefined;
    if (loading.value) return t("admin.gatewayRequestLogs.ipView.loading");
    if (loadError.value || !ipGroups.value) return "";
    return t(
      hasFilters.value
        ? "admin.gatewayRequestLogs.ipView.filteredSummary"
        : "admin.gatewayRequestLogs.ipView.summary",
      {
        date: selectedDate.value,
        ips: ipGroups.value.total_ips,
        requests: ipGroups.value.total_requests,
      },
    );
  });

  const setRootRef = (value: unknown) => {
    root.value = value instanceof HTMLElement ? value : null;
  };
  return { setRootRef, handleOpenIp, ipLocation, ipSummary };
};
