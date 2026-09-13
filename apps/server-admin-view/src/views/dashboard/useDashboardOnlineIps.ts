import { computed, onBeforeUnmount, ref, watch, type Ref } from "vue";
import { browserT } from "@fn-knock/i18n/vue/admin";
import { DashboardAPI } from "@/lib/api/dashboard";
import { useIpLocationBatch } from "@/composables/useIpLocationBatch";
import { getIpLocationText } from "@/composables/ipLocationDisplay";
import type { DashboardOnlineIpsPayload } from "@/types";

// Go emits RFC3339Nano. Date.parse alone would collapse distinct activities
// within the same millisecond and incorrectly fall back to IP ordering.
const fractionalNanoseconds = (timestamp: string) =>
  Number((timestamp.match(/\.(\d+)/)?.[1] ?? "").padEnd(9, "0"));

export const useDashboardOnlineIps = (open: Ref<boolean>) => {
  const snapshot = ref<DashboardOnlineIpsPayload | null>(null);
  const loading = ref(false);
  const error = ref("");
  const page = ref(1);
  const limit = ref("20");
  const order = ref<"desc" | "asc">("desc");
  const pageSize = computed(() => Number(limit.value));
  const locations = useIpLocationBatch({ reuseResolved: true });
  let runId = 0;
  let controller: AbortController | null = null;

  const sortedItems = computed(() =>
    [...(snapshot.value?.items ?? [])].sort((a, b) => {
      const difference =
        Date.parse(a.last_seen_at) - Date.parse(b.last_seen_at) ||
        fractionalNanoseconds(a.last_seen_at) -
          fractionalNanoseconds(b.last_seen_at);
      return (
        (order.value === "desc" ? -difference : difference) ||
        (a.ip < b.ip ? -1 : a.ip > b.ip ? 1 : 0)
      );
    }),
  );
  const visibleItems = computed(() =>
    sortedItems.value.slice(
      (page.value - 1) * pageSize.value,
      page.value * pageSize.value,
    ),
  );
  const displayItems = computed(() =>
    visibleItems.value.map((item) => ({
      ...item,
      locationText: item.ip
        ? getIpLocationText(locations.getSnapshot(item.ip))
        : "—",
    })),
  );
  const ipCount = computed(
    () => snapshot.value?.items.filter((item) => item.ip).length ?? 0,
  );

  const cancel = () => {
    runId++;
    controller?.abort();
    controller = null;
    loading.value = false;
    locations.trackIps([]);
  };

  const refresh = async () => {
    if (!open.value) return;
    controller?.abort();
    controller = new AbortController();
    const currentRun = ++runId;
    loading.value = true;
    error.value = "";
    try {
      const result = await DashboardAPI.getOnlineIps(controller.signal);
      if (currentRun !== runId || !open.value) return;
      snapshot.value = result;
      page.value = 1;
    } catch (caught: unknown) {
      if (currentRun !== runId || !open.value) return;
      const apiError = caught as { response?: { data?: { message?: string } } };
      error.value =
        apiError.response?.data?.message ||
        browserT("admin.dashboard.onlineIps.loadFailed");
    } finally {
      if (currentRun === runId) loading.value = false;
    }
  };

  watch(
    [order, limit],
    () => {
      page.value = 1;
    },
    { flush: "sync" },
  );
  watch(
    [open, visibleItems],
    ([isOpen, items]) => {
      locations.trackIps(
        isOpen ? items.map((item) => item.ip).filter(Boolean) : [],
      );
    },
    { immediate: true },
  );
  watch(
    open,
    (isOpen) => {
      cancel();
      if (isOpen) {
        snapshot.value = null;
        page.value = 1;
        void refresh();
      }
    },
    { immediate: true, flush: "sync" },
  );
  onBeforeUnmount(cancel);

  return {
    snapshot,
    loading,
    error,
    page,
    limit,
    order,
    pageSize,
    displayItems,
    ipCount,
    refresh,
  };
};
