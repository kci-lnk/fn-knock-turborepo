import { computed, ref, watch, type Ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useStorage } from "@vueuse/core";
import type {
  GatewayLogIpGroups,
  GatewayLogIpGroupsQuery,
} from "@/lib/api/gateway";
import type {
  GatewayStatusFilterValue,
  GatewayLoginFilterValue,
  GatewayWAFFilterValue,
} from "./model";

interface IpViewContext {
  loading: Ref<boolean>;
  selectedDate: Ref<string>;
  searchQuery: Ref<string>;
  selectedStatus: Ref<GatewayStatusFilterValue>;
  selectedLoggedIn: Ref<GatewayLoginFilterValue>;
  selectedCredential: Ref<string>;
  selectedWAFStatus: Ref<GatewayWAFFilterValue>;
  limit: Ref<string>;
  selectedLogEntryKeys: Ref<Set<string>>;
  resetCursorPagination: () => void;
  invalidateRequest: () => void;
  trackIps: (ips: string[]) => void;
  fetchEntries: () => Promise<void>;
  applyFilter: (update: () => void) => Promise<void>;
}

export const useGatewayLogIpView = (context: IpViewContext) => {
  const {
    selectedDate,
    searchQuery,
    selectedStatus,
    selectedLoggedIn,
    selectedCredential,
    selectedWAFStatus,
    limit,
    selectedLogEntryKeys,
    resetCursorPagination,
    invalidateRequest,
    trackIps,
    fetchEntries,
    applyFilter,
  } = context;
  const route = useRoute();
  const router = useRouter();
  const viewMode = useStorage<"requests" | "ips">(
    "gateway-log-view",
    "requests",
  );
  const clientIp = computed(() =>
    typeof route.query.client_ip === "string" ? route.query.client_ip : "",
  );
  const isIpOverview = computed(
    () => viewMode.value === "ips" && !clientIp.value,
  );
  const ipGroups = ref<GatewayLogIpGroups | null>(null);
  const ipPage = ref(1);
  const ipSort = ref<NonNullable<GatewayLogIpGroupsQuery["sort"]>>("requests");
  const detailRequests = ref<number | null>(null);
  const loadError = ref(false);
  const hasFilters = computed(
    () =>
      Boolean(searchQuery.value.trim()) ||
      selectedStatus.value !== "all" ||
      selectedLoggedIn.value !== "all" ||
      selectedCredential.value !== "all" ||
      selectedWAFStatus.value !== "all",
  );
  const filtersSnapshot = () => ({
    date: selectedDate.value,
    search: searchQuery.value,
    status: selectedStatus.value,
    loggedIn: selectedLoggedIn.value,
    credential: selectedCredential.value,
    waf: selectedWAFStatus.value,
    limit: limit.value,
  });
  let overviewSnapshot: {
    filters: ReturnType<typeof filtersSnapshot>;
    groups: GatewayLogIpGroups | null;
    page: number;
  } | null = null;
  let openedFromOverview = false;
  const openIp = async (ip: string) => {
    overviewSnapshot = {
      filters: filtersSnapshot(),
      groups: ipGroups.value,
      page: ipPage.value,
    };
    openedFromOverview = true;
    await router.push({
      query: {
        ...route.query,
        client_ip: ip || "unknown",
        log_date: selectedDate.value,
      },
    });
  };
  const backToIps = async () => {
    if (openedFromOverview) {
      router.back();
      return;
    }
    const query = { ...route.query };
    delete query.client_ip;
    delete query.log_date;
    viewMode.value = "ips";
    await router.replace({ query });
  };
  const showAllIpRequests = () =>
    applyFilter(() => {
      searchQuery.value = "";
      selectedStatus.value = "all";
      selectedLoggedIn.value = "all";
      selectedCredential.value = "all";
      selectedWAFStatus.value = "all";
    });
  const setViewMode = async (value: "requests" | "ips") => {
    if (viewMode.value === value) return;
    viewMode.value = value;
    selectedLogEntryKeys.value = new Set();
    await applyFilter(() => undefined);
  };
  const handleIpSortChange = (value: unknown) =>
    applyFilter(() => {
      ipSort.value = String(value) as typeof ipSort.value;
    });
  watch(
    [clientIp, () => route.query.log_date],
    async ([ip, date], [previous, previousDate]) => {
      if (
        ip === previous &&
        (!ip || date === previousDate || date === selectedDate.value)
      )
        return;
      invalidateRequest();
      loadError.value = false;
      selectedLogEntryKeys.value = new Set();
      resetCursorPagination();
      detailRequests.value = null;
      if (!ip) {
        openedFromOverview = false;
        viewMode.value = "ips";
        if (overviewSnapshot) {
          const { filters, groups, page } = overviewSnapshot;
          selectedDate.value = filters.date;
          searchQuery.value = filters.search;
          selectedStatus.value = filters.status;
          selectedLoggedIn.value = filters.loggedIn;
          selectedCredential.value = filters.credential;
          selectedWAFStatus.value = filters.waf;
          limit.value = filters.limit;
          ipGroups.value = groups;
          ipPage.value = page;
          trackIps(groups?.items.map((item) => item.client_ip) ?? []);
          return;
        }
      } else if (typeof route.query.log_date === "string") {
        selectedDate.value = route.query.log_date;
      }
      if (
        ip &&
        overviewSnapshot &&
        JSON.stringify(filtersSnapshot()) ===
          JSON.stringify(overviewSnapshot.filters)
      ) {
        detailRequests.value =
          overviewSnapshot.groups?.items.find(
            (item) => (item.client_ip || "unknown") === ip,
          )?.requests ?? null;
      }
      await fetchEntries();
    },
  );

  const syncDateQuery = async () => {
    if (clientIp.value)
      await router.replace({
        query: { ...route.query, log_date: selectedDate.value },
      });
  };
  const loadPage = async (
    direction: "first" | "newer" | "older",
    loadEntries: () => boolean,
  ) => {
    if (context.loading.value) return;
    if (isIpOverview.value) {
      if (
        direction === "older" &&
        ipPage.value * Number(limit.value) >= (ipGroups.value?.total ?? 0)
      )
        return;
      if (direction === "newer" && ipPage.value <= 1) return;
      ipPage.value =
        direction === "first"
          ? 1
          : ipPage.value + (direction === "older" ? 1 : -1);
      await fetchEntries();
    } else if (loadEntries()) await fetchEntries();
  };
  const invalidateOverview = () => {
    detailRequests.value = null;
    overviewSnapshot = null;
    ipGroups.value = null;
    ipPage.value = 1;
  };
  return {
    syncDateQuery,
    loadPage,
    viewMode,
    clientIp,
    isIpOverview,
    ipGroups,
    ipPage,
    ipSort,
    detailRequests,
    loadError,
    hasFilters,
    openIp,
    backToIps,
    showAllIpRequests,
    setViewMode,
    handleIpSortChange,
    invalidateOverview,
  };
};
