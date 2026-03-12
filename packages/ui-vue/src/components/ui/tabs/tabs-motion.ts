import type { ComputedRef, InjectionKey, Ref } from "vue";
import { inject } from "vue";

export type TabsValue = string | number;
export type TabsMotionDirection = "from-start" | "from-end" | "none";

export interface TabsMotionContext {
  modelValue: Ref<TabsValue | undefined>;
  previousValue: Ref<TabsValue | undefined>;
  motionDirection: ComputedRef<TabsMotionDirection>;
  registerContent: (value: TabsValue) => void;
  unregisterContent: (value: TabsValue) => void;
}

export const tabsMotionContextKey: InjectionKey<TabsMotionContext> =
  Symbol("tabs-motion");

export function useTabsMotionContext() {
  return inject(tabsMotionContextKey, null);
}
