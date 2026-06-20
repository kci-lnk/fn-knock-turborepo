import { ref } from "vue";
import type { HostMapping } from "@/types";
import { getFaviconKey } from "./model";

export const useMappingFaviconState = () => {
  const brokenFaviconKeys = ref(new Set<string>());

  const isFaviconBroken = (mapping: HostMapping): boolean =>
    brokenFaviconKeys.value.has(getFaviconKey(mapping));

  const markFaviconBroken = (mapping: HostMapping) => {
    const next = new Set(brokenFaviconKeys.value);
    next.add(getFaviconKey(mapping));
    brokenFaviconKeys.value = next;
  };

  const resetFaviconErrors = () => {
    brokenFaviconKeys.value = new Set();
  };

  return {
    isFaviconBroken,
    markFaviconBroken,
    resetFaviconErrors,
  };
};
