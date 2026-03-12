import { onMounted, ref, toValue, watch, type MaybeRefOrGetter } from 'vue';

type QueryScalar = string | null;
type QueryValue = QueryScalar | QueryScalar[] | undefined;

interface RouteLike {
  query: Record<string, QueryValue>;
}

interface RouterLike {
  replace: (args: { query: Record<string, QueryValue> }) => unknown;
}

interface UseSyncedQueryTabOptions {
  route: RouteLike;
  router: RouterLike;
  defaultTab: MaybeRefOrGetter<string>;
  allowedTabs: MaybeRefOrGetter<Iterable<string>>;
  queryKey?: string;
  active?: MaybeRefOrGetter<boolean>;
}

const readFirstQueryValue = (value: QueryValue) => {
  if (Array.isArray(value)) return value[0] || '';
  return value || '';
};

export function useSyncedQueryTab(options: UseSyncedQueryTabOptions) {
  const queryKey = options.queryKey ?? 'tab';
  const currentTab = ref(toValue(options.defaultTab));
  const isActive = () => toValue(options.active ?? true);

  const getAllowedTabs = () => new Set(Array.from(toValue(options.allowedTabs)));

  const normalizeTab = (value: string) => {
    const fallback = toValue(options.defaultTab);
    const allowed = getAllowedTabs();
    return allowed.has(value) ? value : fallback;
  };

  const getQueryTabValue = () => readFirstQueryValue(options.route.query[queryKey]);

  const syncQueryTab = (tab: string) => {
    const normalizedTab = normalizeTab(tab);
    const defaultTab = toValue(options.defaultTab);
    const normalizedQuery = normalizedTab === defaultTab ? '' : normalizedTab;
    if (getQueryTabValue() === normalizedQuery) return;

    const nextQuery: Record<string, QueryValue> = { ...options.route.query };
    if (!normalizedQuery) {
      delete nextQuery[queryKey];
    } else {
      nextQuery[queryKey] = normalizedQuery;
    }
    options.router.replace({ query: nextQuery });
  };

  const syncFromRoute = () => {
    if (!isActive()) return;
    const queryTab = getQueryTabValue();
    const next = normalizeTab(queryTab || toValue(options.defaultTab));
    currentTab.value = next;
    syncQueryTab(next);
  };

  const navigateTo = (tab: string | number) => {
    const next = String(tab);
    if (!next || next === currentTab.value) return;
    const normalized = normalizeTab(next);
    currentTab.value = normalized;
    syncQueryTab(normalized);
  };

  const allowedTabsSignature = () => Array.from(getAllowedTabs()).join('|');

  onMounted(() => {
    if (isActive()) {
      syncFromRoute();
    }
  });
  watch(
    [() => options.route.query[queryKey], () => toValue(options.defaultTab), allowedTabsSignature, isActive],
    () => {
      if (isActive()) {
        syncFromRoute();
      }
    },
  );

  return {
    currentTab,
    navigateTo,
    syncFromRoute,
    syncQueryTab,
  };
}
