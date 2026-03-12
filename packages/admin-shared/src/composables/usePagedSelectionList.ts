import { computed, ref, shallowRef } from 'vue';
import { watchDebounced } from '@vueuse/core';

export type PagedQueryParams = {
  page: number;
  limit: string;
  query: string;
};

export type PagedQueryResult<TItem> = {
  items: TItem[];
  total: number;
};

interface UsePagedSelectionListOptions<TItem, TKey extends string> {
  fetchPage: (params: PagedQueryParams) => Promise<PagedQueryResult<TItem>>;
  getKey: (item: TItem) => TKey;
  onError: (error: unknown) => void;
  initialLimit?: string;
  debounce?: number;
  clampPageOnOverflow?: boolean;
}

export function usePagedSelectionList<TItem, TKey extends string>(
  options: UsePagedSelectionListOptions<TItem, TKey>,
) {
  const items = shallowRef<TItem[]>([]);
  const total = ref(0);
  const loading = ref(false);
  const searchQuery = ref('');
  const currentPage = ref(1);
  const limit = ref(options.initialLimit ?? '20');
  const selectedKeys = shallowRef<Set<TKey>>(new Set());

  const parsedLimit = computed(() => {
    const value = Number.parseInt(limit.value, 10);
    return Number.isFinite(value) && value > 0 ? value : 20;
  });

  const totalPages = computed(() => Math.max(1, Math.ceil(total.value / parsedLimit.value)));

  const clearSelection = () => {
    selectedKeys.value = new Set();
  };

  const toggleSelect = (key: TKey) => {
    if (selectedKeys.value.has(key)) {
      selectedKeys.value.delete(key);
    } else {
      selectedKeys.value.add(key);
    }
    selectedKeys.value = new Set(selectedKeys.value);
  };

  const isAllSelected = computed({
    get: () =>
      items.value.length > 0 &&
      items.value.every((item) => selectedKeys.value.has(options.getKey(item))),
    set: (checked: boolean) => {
      if (checked) {
        items.value.forEach((item) => {
          selectedKeys.value.add(options.getKey(item));
        });
      } else {
        items.value.forEach((item) => {
          selectedKeys.value.delete(options.getKey(item));
        });
      }
      selectedKeys.value = new Set(selectedKeys.value);
    },
  });

  const fetchList = async (): Promise<void> => {
    loading.value = true;
    try {
      const result = await options.fetchPage({
        page: currentPage.value,
        limit: limit.value,
        query: searchQuery.value,
      });
      items.value = result.items;
      total.value = result.total;

      if ((options.clampPageOnOverflow ?? true) && currentPage.value > totalPages.value) {
        currentPage.value = totalPages.value;
        return fetchList();
      }
    } catch (error) {
      options.onError(error);
    } finally {
      loading.value = false;
    }
  };

  const handleSearch = () => {
    currentPage.value = 1;
    fetchList();
  };

  const handlePageChange = (page: number) => {
    if (page < 1 || page > totalPages.value) return;
    currentPage.value = page;
    fetchList();
  };

  const handleLimitChange = (newLimit: unknown) => {
    if (newLimit === null || newLimit === undefined) return;
    limit.value = String(newLimit);
    currentPage.value = 1;
    fetchList();
  };

  watchDebounced(
    searchQuery,
    () => {
      handleSearch();
    },
    { debounce: options.debounce ?? 500 },
  );

  return {
    items,
    total,
    loading,
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    totalPages,
    selectedKeys,
    isAllSelected,
    fetchList,
    handleSearch,
    handlePageChange,
    handleLimitChange,
    toggleSelect,
    clearSelection,
  };
}
