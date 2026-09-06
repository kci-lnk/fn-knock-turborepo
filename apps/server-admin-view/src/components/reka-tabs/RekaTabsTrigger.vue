<script setup lang="ts">
import type { TabsTriggerProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import {
  TabsTrigger as RekaTabsTriggerPrimitive,
  useForwardProps,
} from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<
  TabsTriggerProps & { class?: HTMLAttributes["class"] }
>();

const delegatedProps = reactiveOmit(props, "class");
const forwardedProps = useForwardProps(delegatedProps);
</script>

<template>
  <RekaTabsTriggerPrimitive
    data-slot="reka-tabs-trigger"
    v-bind="forwardedProps"
    :class="
      cn(
        'relative z-10 inline-flex select-none [-webkit-user-select:none] [-webkit-touch-callout:none] min-h-11 min-w-[120px] flex-none items-center justify-center whitespace-nowrap rounded-none border-b-2 border-transparent px-6 py-2 text-sm font-medium text-muted-foreground transition-[color] duration-200 outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-emerald-500/25 focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50 data-[state=active]:text-emerald-600',
        props.class,
      )
    "
  >
    <slot />
  </RekaTabsTriggerPrimitive>
</template>
