<script setup lang="ts">
import type { TabsListProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import {
  TabsIndicator as RekaTabsIndicatorPrimitive,
  TabsList as RekaTabsListPrimitive,
  useForwardProps,
} from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<
  TabsListProps & {
    class?: HTMLAttributes["class"];
    indicatorClass?: HTMLAttributes["class"];
  }
>();

const delegatedProps = reactiveOmit(props, "class", "indicatorClass");
const forwardedProps = useForwardProps(delegatedProps);
</script>

<template>
  <RekaTabsListPrimitive
    data-slot="reka-tabs-list"
    v-bind="forwardedProps"
    :class="
      cn(
        'relative inline-flex max-w-full items-stretch overflow-x-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden rounded-lg border border-border/80 bg-background px-3 pt-0 shadow-none after:pointer-events-none after:absolute after:right-3 after:bottom-0 after:left-3 after:h-px after:bg-border/80',
        props.class,
      )
    "
  >
    <RekaTabsIndicatorPrimitive
      data-slot="reka-tabs-indicator"
      :class="
        cn(
          'absolute bottom-0 left-0 z-0 h-0.5 rounded-full bg-emerald-500 transition-[width,transform] duration-300 ease-out',
          props.indicatorClass,
        )
      "
      :style="{
        width: 'var(--reka-tabs-indicator-size)',
        transform: 'translateX(var(--reka-tabs-indicator-position))',
      }"
    />
    <slot />
  </RekaTabsListPrimitive>
</template>
