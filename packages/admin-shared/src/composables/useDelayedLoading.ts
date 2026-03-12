import { computed, onBeforeUnmount, ref, toValue, watch, type MaybeRefOrGetter } from 'vue';

interface UseDelayedLoadingOptions {
  delay?: number;
}

export function useDelayedLoading(
  source: MaybeRefOrGetter<boolean>,
  options: UseDelayedLoadingOptions = {},
) {
  const delay = options.delay ?? 220;
  const visible = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  const clearTimer = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  };

  watch(
    () => toValue(source),
    (loading) => {
      clearTimer();
      if (!loading) {
        visible.value = false;
        return;
      }

      timer = setTimeout(() => {
        visible.value = true;
        timer = null;
      }, delay);
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    clearTimer();
  });

  return computed(() => visible.value);
}
