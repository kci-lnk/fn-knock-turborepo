<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { VueDraggable } from "vue-draggable-plus";
import type { HostMapping } from "@/types";

const props = defineProps<{
  collapsed: boolean;
  disabled: boolean;
  emptyLabel: string;
  mappings: HostMapping[];
  showHeader: boolean;
}>();

const emit = defineEmits<{
  end: [];
  "update:mappings": [mappings: HostMapping[]];
}>();

const model = computed({
  get: () => props.mappings,
  set: (value: HostMapping[]) => emit("update:mappings", value),
});

const isBodyRendered = ref(!props.collapsed);
const isBodyVisuallyCollapsed = ref(props.collapsed);
let animationFrame = 0;
let collapseFallbackTimer: ReturnType<typeof setTimeout> | null = null;

const cancelPendingAnimation = () => {
  if (animationFrame) {
    cancelAnimationFrame(animationFrame);
    animationFrame = 0;
  }
  if (collapseFallbackTimer) {
    clearTimeout(collapseFallbackTimer);
    collapseFallbackTimer = null;
  }
};

const finishCollapse = () => {
  if (!props.collapsed) return;
  isBodyRendered.value = false;
  collapseFallbackTimer = null;
};

const beginExpand = async () => {
  isBodyRendered.value = true;
  isBodyVisuallyCollapsed.value = true;
  await nextTick();
  if (props.collapsed) return;

  animationFrame = requestAnimationFrame(() => {
    animationFrame = requestAnimationFrame(() => {
      animationFrame = 0;
      if (!props.collapsed) isBodyVisuallyCollapsed.value = false;
    });
  });
};

watch(
  () => props.collapsed,
  (collapsed) => {
    cancelPendingAnimation();
    if (collapsed) {
      isBodyVisuallyCollapsed.value = true;
      collapseFallbackTimer = setTimeout(finishCollapse, 260);
      return;
    }
    void beginExpand();
  },
);

const handleBodyTransitionEnd = (event: TransitionEvent) => {
  if (
    event.target !== event.currentTarget ||
    event.propertyName !== "clip-path"
  ) {
    return;
  }
  if (props.collapsed) {
    cancelPendingAnimation();
    finishCollapse();
  }
};

onBeforeUnmount(cancelPendingAnimation);
</script>

<template>
  <tbody v-if="showHeader">
    <slot name="header" />
  </tbody>
  <VueDraggable
    v-if="isBodyRendered"
    v-model="model"
    tag="tbody"
    :class="[
      'mapping-group-collapse-body [&_tr:last-child]:border-0',
      {
        'mapping-group-collapse-body--collapsed': isBodyVisuallyCollapsed,
      },
    ]"
    :inert="isBodyVisuallyCollapsed"
    handle=".mapping-drag-handle"
    draggable=".mapping-row"
    ghost-class="bg-muted/60"
    chosen-class="bg-muted/80"
    :animation="180"
    :disabled="disabled || isBodyVisuallyCollapsed"
    :group="{ name: 'host-mapping-groups', pull: true, put: true }"
    @transitionend="handleBodyTransitionEnd"
    @end="emit('end')"
  >
    <tr v-if="mappings.length === 0" class="h-14">
      <td colspan="8" class="text-center text-xs text-muted-foreground">
        {{ emptyLabel }}
      </td>
    </tr>
    <slot v-for="mapping in model" :key="mapping.host" :mapping="mapping" />
  </VueDraggable>
</template>
