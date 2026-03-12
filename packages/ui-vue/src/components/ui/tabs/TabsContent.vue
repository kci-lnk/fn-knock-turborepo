<script setup lang="ts">
import type { TabsContentProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { computed, onBeforeUnmount, onMounted } from "vue";
import { TabsContent } from "reka-ui";
import { cn } from "@/lib/utils";
import { useTabsMotionContext } from "./tabs-motion";

const props = defineProps<
  TabsContentProps & { class?: HTMLAttributes["class"] }
>();

const delegatedProps = reactiveOmit(props, "class");
const tabsMotion = useTabsMotionContext();

const motionClasses = computed(() => {
  switch (tabsMotion?.motionDirection.value) {
    case "from-end":
      return "data-[state=active]:slide-in-from-right-4 data-[state=inactive]:slide-out-to-left-4";
    case "from-start":
      return "data-[state=active]:slide-in-from-left-4 data-[state=inactive]:slide-out-to-right-4";
    default:
      return "";
  }
});

onMounted(() => {
  tabsMotion?.registerContent(props.value);
});

onBeforeUnmount(() => {
  tabsMotion?.unregisterContent(props.value);
});
</script>

<template>
  <TabsContent
    data-slot="tabs-content"
    :class="
      cn(
        'flex-1 outline-none motion-safe:data-[state=active]:animate-in motion-safe:data-[state=inactive]:animate-out motion-safe:data-[state=active]:fade-in-0 motion-safe:data-[state=inactive]:fade-out-0 motion-safe:data-[state=active]:duration-300 motion-safe:data-[state=inactive]:duration-200 motion-safe:data-[state=active]:ease-out motion-safe:data-[state=inactive]:ease-in motion-safe:will-change-[transform,opacity]',
        motionClasses,
        props.class,
      )
    "
    :data-motion="tabsMotion?.motionDirection.value ?? 'none'"
    v-bind="delegatedProps"
  >
    <slot />
  </TabsContent>
</template>
