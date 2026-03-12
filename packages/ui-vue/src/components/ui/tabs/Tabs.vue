<script setup lang="ts">
import type { TabsRootEmits, TabsRootProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit, useVModel } from "@vueuse/core";
import { computed, provide, shallowRef } from "vue";
import { TabsRoot, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";
import { tabsMotionContextKey, type TabsValue } from "./tabs-motion";

const props = defineProps<
  TabsRootProps & { class?: HTMLAttributes["class"] }
>();
const emits = defineEmits<TabsRootEmits>();

const delegatedProps = reactiveOmit(
  props,
  "class",
  "defaultValue",
  "modelValue",
);
const forwarded = useForwardProps(delegatedProps);

const modelValue = useVModel(props, "modelValue", emits, {
  passive: true,
  defaultValue: props.defaultValue,
});

const previousValue = shallowRef<TabsValue | undefined>(undefined);
const contentOrder = shallowRef<TabsValue[]>([]);

function registerContent(value: TabsValue) {
  if (contentOrder.value.includes(value)) {
    return;
  }

  contentOrder.value = [...contentOrder.value, value];
}

function unregisterContent(value: TabsValue) {
  contentOrder.value = contentOrder.value.filter((item) => item !== value);
}

const motionDirection = computed(() => {
  const currentIndex = contentOrder.value.indexOf(
    modelValue.value as TabsValue,
  );
  const previousIndex = contentOrder.value.indexOf(
    previousValue.value as TabsValue,
  );

  if (
    currentIndex === -1 ||
    previousIndex === -1 ||
    currentIndex === previousIndex
  ) {
    return "none";
  }

  return currentIndex > previousIndex ? "from-end" : "from-start";
});

function handleUpdateModelValue(value: string | number) {
  if (value !== modelValue.value) {
    previousValue.value = modelValue.value as TabsValue | undefined;
  }

  modelValue.value = value;
}

provide(tabsMotionContextKey, {
  modelValue,
  previousValue,
  motionDirection,
  registerContent,
  unregisterContent,
});
</script>

<template>
  <TabsRoot
    v-slot="slotProps"
    data-slot="tabs"
    :model-value="modelValue"
    v-bind="forwarded"
    @update:model-value="handleUpdateModelValue"
    :class="cn('flex flex-col gap-2', props.class)"
  >
    <slot v-bind="slotProps" />
  </TabsRoot>
</template>
