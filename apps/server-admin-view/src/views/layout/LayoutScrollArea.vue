<template>
  <nav class="layout-scroll-area">
    <div
      ref="viewport"
      class="layout-scroll-area__viewport"
      :class="[
        contentClass,
        {
          'layout-scroll-area__viewport--rail-gutter': reserveRailGutter,
        },
      ]"
      @scroll.passive="handleScroll"
    >
      <slot />
    </div>
    <div
      v-if="isOverflowing"
      class="layout-scroll-area__rail"
      :class="{ 'layout-scroll-area__rail--visible': isTemporarilyVisible }"
      aria-hidden="true"
      @pointerdown="handleRailPointerDown"
    >
      <div
        class="layout-scroll-area__thumb"
        :style="thumbStyle"
        @pointerdown.stop="handleThumbPointerDown"
        @pointermove="handleThumbPointerMove"
        @pointerup="finishThumbDrag"
        @pointercancel="finishThumbDrag"
      ></div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  type CSSProperties,
} from "vue";

const props = withDefaults(
  defineProps<{
    contentClass?: string;
    hintOnMount?: boolean;
    reserveRailGutter?: boolean;
  }>(),
  {
    contentClass: "",
    hintOnMount: false,
    reserveRailGutter: false,
  },
);

const viewport = ref<HTMLElement>();
const isOverflowing = ref(false);
const isHinting = ref(false);
const isScrolling = ref(false);
const isDragging = ref(false);
const thumbHeight = ref(28);
const thumbOffset = ref(0);
let hasHinted = false;
let hintTimer: number | undefined;
let scrollTimer: number | undefined;
let resizeObserver: ResizeObserver | undefined;
let mutationObserver: MutationObserver | undefined;
let dragStartY = 0;
let dragStartScrollTop = 0;

const isTemporarilyVisible = computed(
  () => isHinting.value || isScrolling.value || isDragging.value,
);
const thumbStyle = computed<CSSProperties>(() => ({
  height: `${thumbHeight.value}px`,
  transform: `translate3d(0, ${thumbOffset.value}px, 0)`,
}));

const revealMountHint = () => {
  if (!props.hintOnMount || hasHinted) return;
  hasHinted = true;
  isHinting.value = true;
  hintTimer = window.setTimeout(() => {
    isHinting.value = false;
  }, 1600);
};

const syncThumb = () => {
  const element = viewport.value;
  if (!element) return;
  const { clientHeight, scrollHeight, scrollTop } = element;
  const nextOverflowing = scrollHeight > clientHeight + 1;
  isOverflowing.value = nextOverflowing;
  if (!nextOverflowing) return;

  const railHeight = Math.max(0, clientHeight - 8);
  const nextThumbHeight = Math.max(
    28,
    Math.round((clientHeight / scrollHeight) * railHeight),
  );
  const availableTravel = Math.max(0, railHeight - nextThumbHeight);
  const maxScrollTop = scrollHeight - clientHeight;
  thumbHeight.value = nextThumbHeight;
  thumbOffset.value = Math.round((scrollTop / maxScrollTop) * availableTravel);
  revealMountHint();
};

const handleScroll = () => {
  syncThumb();
  isScrolling.value = true;
  window.clearTimeout(scrollTimer);
  scrollTimer = window.setTimeout(() => {
    isScrolling.value = false;
  }, 700);
};

const handleRailPointerDown = (event: PointerEvent) => {
  const element = viewport.value;
  if (!element || event.target !== event.currentTarget) return;
  const rail = event.currentTarget as HTMLElement;
  const ratio = Math.min(
    1,
    Math.max(0, (event.clientY - rail.getBoundingClientRect().top) / rail.clientHeight),
  );
  element.scrollTo({
    top: ratio * (element.scrollHeight - element.clientHeight),
    behavior: "smooth",
  });
};

const handleThumbPointerDown = (event: PointerEvent) => {
  const element = viewport.value;
  if (!element || (event.pointerType === "mouse" && event.button !== 0)) return;
  isDragging.value = true;
  dragStartY = event.clientY;
  dragStartScrollTop = element.scrollTop;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
};

const handleThumbPointerMove = (event: PointerEvent) => {
  const element = viewport.value;
  if (!element || !isDragging.value) return;
  const maxScrollTop = element.scrollHeight - element.clientHeight;
  const availableTravel = element.clientHeight - 8 - thumbHeight.value;
  if (availableTravel <= 0) return;
  element.scrollTop =
    dragStartScrollTop +
    ((event.clientY - dragStartY) / availableTravel) * maxScrollTop;
};

const finishThumbDrag = (event: PointerEvent) => {
  isDragging.value = false;
  const thumb = event.currentTarget as HTMLElement;
  if (thumb.hasPointerCapture(event.pointerId)) {
    thumb.releasePointerCapture(event.pointerId);
  }
};

onMounted(() => {
  void nextTick(syncThumb);
  window.addEventListener("resize", syncThumb);
  if (typeof ResizeObserver !== "undefined" && viewport.value) {
    resizeObserver = new ResizeObserver(syncThumb);
    resizeObserver.observe(viewport.value);
  }
  if (typeof MutationObserver !== "undefined" && viewport.value) {
    mutationObserver = new MutationObserver(syncThumb);
    mutationObserver.observe(viewport.value, { childList: true, subtree: true });
  }
});

onBeforeUnmount(() => {
  window.clearTimeout(hintTimer);
  window.clearTimeout(scrollTimer);
  window.removeEventListener("resize", syncThumb);
  resizeObserver?.disconnect();
  mutationObserver?.disconnect();
});
</script>

<style scoped>
.layout-scroll-area {
  position: relative;
  min-height: 0;
}

.layout-scroll-area__viewport {
  height: 100%;
  overflow-y: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.layout-scroll-area__viewport::-webkit-scrollbar {
  display: none;
}

.layout-scroll-area__viewport--rail-gutter {
  padding-inline-end: 12px;
}

.layout-scroll-area__rail {
  position: absolute;
  z-index: 10;
  top: 4px;
  right: 1px;
  bottom: 4px;
  width: 8px;
  border-radius: 999px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 180ms ease;
}

.layout-scroll-area:hover .layout-scroll-area__rail,
.layout-scroll-area:focus-within .layout-scroll-area__rail,
.layout-scroll-area__rail--visible {
  opacity: 1;
  pointer-events: auto;
}

.layout-scroll-area__thumb {
  width: 4px;
  margin-left: auto;
  border-radius: 999px;
  background: rgb(0 0 0 / 16%);
  cursor: grab;
  touch-action: none;
  transition:
    width 160ms ease,
    background-color 160ms ease;
}

.layout-scroll-area__thumb:hover,
.layout-scroll-area__thumb:active {
  width: 6px;
  background: rgb(0 0 0 / 24%);
}

.layout-scroll-area__thumb:active {
  cursor: grabbing;
}

@media (prefers-reduced-motion: reduce) {
  .layout-scroll-area__rail,
  .layout-scroll-area__thumb {
    transition: none;
  }
}
</style>
