import { computed, ref, type Ref } from "vue";

export const useCursorPagination = ({ loading }: { loading: Ref<boolean> }) => {
  const currentCursor = ref("");
  const nextCursor = ref("");
  const cursorHistory = ref<string[]>([]);
  const canLoadNewer = computed(() => cursorHistory.value.length > 0);
  const canLoadOlder = computed(() => Boolean(nextCursor.value));

  const reset = () => {
    currentCursor.value = "";
    nextCursor.value = "";
    cursorHistory.value = [];
  };

  const loadOlder = () => {
    if (!nextCursor.value || loading.value) return false;
    cursorHistory.value = [...cursorHistory.value, currentCursor.value];
    currentCursor.value = nextCursor.value;
    return true;
  };

  const loadNewer = () => {
    if (cursorHistory.value.length === 0 || loading.value) return false;
    const history = [...cursorHistory.value];
    currentCursor.value = history.pop() ?? "";
    cursorHistory.value = history;
    return true;
  };

  const loadFirst = () => {
    if (cursorHistory.value.length === 0 || loading.value) return false;
    reset();
    return true;
  };

  return {
    canLoadNewer,
    canLoadOlder,
    currentCursor,
    cursorHistory,
    loadFirst,
    loadNewer,
    loadOlder,
    nextCursor,
    reset,
  };
};
