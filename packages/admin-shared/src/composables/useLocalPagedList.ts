import { computed, ref, watch, type ComputedRef, type Ref } from 'vue';

type ReadableItems<T> = Ref<T[]> | ComputedRef<T[]>;

interface UseLocalPagedListOptions<T> {
  items: ReadableItems<T>;
  filter: (item: T, query: string) => boolean;
  initialLimit?: string;
  normalizeQuery?: (query: string) => string;
}

export function useLocalPagedList<T>(options: UseLocalPagedListOptions<T>) {
  const searchQuery = ref('');
  const currentPage = ref(1);
  const limit = ref(options.initialLimit ?? '10');

  const parsedLimit = computed(() => {
    const value = Number.parseInt(limit.value, 10);
    return Number.isFinite(value) && value > 0 ? value : 10;
  });

  const normalizedQuery = computed(() => {
    const raw = searchQuery.value;
    if (!raw) return '';
    const normalize = options.normalizeQuery ?? ((q: string) => q);
    return normalize(raw);
  });

  const filteredItems = computed(() => {
    if (!normalizedQuery.value) return options.items.value;
    return options.items.value.filter((item) => options.filter(item, normalizedQuery.value));
  });

  const totalPages = computed(() => Math.max(1, Math.ceil(filteredItems.value.length / parsedLimit.value)));

  const pagedItems = computed(() => {
    const start = (currentPage.value - 1) * parsedLimit.value;
    return filteredItems.value.slice(start, start + parsedLimit.value);
  });

  const handlePageChange = (page: number) => {
    if (page < 1 || page > totalPages.value) return;
    currentPage.value = page;
  };

  const handleLimitChange = (newLimit: string) => {
    limit.value = String(newLimit);
    currentPage.value = 1;
  };

  watch(searchQuery, () => {
    currentPage.value = 1;
  });

  watch(filteredItems, () => {
    if (currentPage.value > totalPages.value) {
      currentPage.value = totalPages.value;
    }
  });

  return {
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    filteredItems,
    pagedItems,
    totalPages,
    handlePageChange,
    handleLimitChange,
  };
}
