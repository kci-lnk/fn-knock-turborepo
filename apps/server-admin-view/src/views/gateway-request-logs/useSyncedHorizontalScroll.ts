import { computed, ref, type ComponentPublicInstance } from "vue";

export const useSyncedHorizontalScroll = () => {
  const tableScrollRef = ref<HTMLElement | null>(null);
  const topScrollbarRef = ref<HTMLElement | null>(null);
  const tableContentWidth = ref(0);
  const tableViewportWidth = ref(0);
  const tableScrollLeft = ref(0);

  let resizeObserver: ResizeObserver | null = null;
  let isSyncingHorizontalScroll = false;

  const hasHorizontalOverflow = computed(
    () => tableContentWidth.value > tableViewportWidth.value + 1,
  );
  const canScrollLeft = computed(
    () => hasHorizontalOverflow.value && tableScrollLeft.value > 1,
  );
  const canScrollRight = computed(
    () =>
      hasHorizontalOverflow.value &&
      tableScrollLeft.value + tableViewportWidth.value <
        tableContentWidth.value - 1,
  );

  const updateHorizontalOverflow = () => {
    const scrollEl = tableScrollRef.value;
    tableViewportWidth.value = scrollEl?.clientWidth || 0;
    tableContentWidth.value = scrollEl?.scrollWidth || 0;
    tableScrollLeft.value = scrollEl?.scrollLeft || 0;
  };

  const syncHorizontalScroll = (source: "table" | "top") => {
    if (isSyncingHorizontalScroll) return;

    const tableEl = tableScrollRef.value;
    const topEl = topScrollbarRef.value;
    if (!tableEl || !topEl) return;

    isSyncingHorizontalScroll = true;
    if (source === "table") {
      topEl.scrollLeft = tableEl.scrollLeft;
    } else {
      tableEl.scrollLeft = topEl.scrollLeft;
    }

    requestAnimationFrame(() => {
      isSyncingHorizontalScroll = false;
    });
  };

  const disposeResizeObserver = () => {
    resizeObserver?.disconnect();
    resizeObserver = null;
  };

  const resolveElement = (
    value: Element | ComponentPublicInstance | null,
  ): HTMLElement | null => (value instanceof HTMLElement ? value : null);

  const setTableScrollRef = (
    value: Element | ComponentPublicInstance | null,
  ) => {
    tableScrollRef.value = resolveElement(value);
  };

  const setTopScrollbarRef = (
    value: Element | ComponentPublicInstance | null,
  ) => {
    topScrollbarRef.value = resolveElement(value);
  };

  const bindResizeObserver = () => {
    disposeResizeObserver();

    if (typeof ResizeObserver === "undefined" || !tableScrollRef.value) {
      updateHorizontalOverflow();
      return;
    }

    resizeObserver = new ResizeObserver(() => {
      updateHorizontalOverflow();
    });

    resizeObserver.observe(tableScrollRef.value);

    const tableEl = tableScrollRef.value.querySelector("table");
    if (tableEl instanceof HTMLElement) {
      resizeObserver.observe(tableEl);
    }

    updateHorizontalOverflow();
  };

  return {
    bindResizeObserver,
    canScrollLeft,
    canScrollRight,
    disposeResizeObserver,
    hasHorizontalOverflow,
    setTableScrollRef,
    setTopScrollbarRef,
    syncHorizontalScroll,
    tableContentWidth,
    tableViewportWidth,
    updateHorizontalOverflow,
  };
};
