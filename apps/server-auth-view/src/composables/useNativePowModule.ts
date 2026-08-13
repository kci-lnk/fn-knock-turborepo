import { ref, watch, type Ref } from "vue";

export function useNativePowModule(
  provider: Readonly<Ref<string | null>>,
  supported: Readonly<Ref<boolean>>,
) {
  const ready = ref(false);

  watch(
    [provider, supported],
    async ([activeProvider, isSupported]) => {
      if (activeProvider !== "pow" || !isSupported || ready.value) return;
      try {
        await import("altcha");
        ready.value = true;
      } catch {
        ready.value = false;
      }
    },
    { immediate: true },
  );

  return ready;
}
