<script setup lang="ts">
import {
  computed,
  nextTick,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  ref,
  useSlots,
  watch,
  type HTMLAttributes,
} from "vue";
import { cn } from "@/lib/utils";

type DockAlign = "end" | "center";
type DockVariant = "actions" | "surface";

interface Props {
  active: boolean;
  align?: DockAlign;
  variant?: DockVariant;
  keepVisible?: boolean;
  keepVisibleReleaseDelay?: number;
  visibleThreshold?: number;
  inlineClass?: HTMLAttributes["class"];
  floatingClass?: HTMLAttributes["class"];
  ariaLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  align: "center",
  variant: "actions",
  keepVisible: false,
  keepVisibleReleaseDelay: 320,
  visibleThreshold: 0.25,
  inlineClass: "",
  floatingClass: "",
  ariaLabel: "Floating actions",
});

const slots = useSlots();
const inlineRef = ref<HTMLElement | null>(null);
const floatingRef = ref<HTMLElement | null>(null);
const isInlineVisible = ref(true);
const hasFloatingFocus = ref(false);
const isKeepVisibleReleasing = ref(false);
const isLifecycleActive = ref(true);

let intersectionObserver: IntersectionObserver | null = null;
let keepVisibleReleaseTimer: number | null = null;

const isDockActive = computed(
  () =>
    isLifecycleActive.value &&
    (props.active || props.keepVisible || isKeepVisibleReleasing.value),
);
const shouldShowFloating = computed(
  () =>
    isDockActive.value &&
    (props.keepVisible ||
      isKeepVisibleReleasing.value ||
      !isInlineVisible.value),
);
const shouldRenderFloating = computed(
  () =>
    shouldShowFloating.value || (isDockActive.value && hasFloatingFocus.value),
);

const inlineClasses = computed(() => cn("w-full", props.inlineClass));
const floatingFrameClasses = computed(() =>
  cn(
    "fixed inset-x-0 bottom-0 z-40 box-border flex justify-center pointer-events-none px-4 pb-[calc(env(safe-area-inset-bottom)+0.875rem)]",
  ),
);
const floatingPanelClasses = computed(() =>
  cn(
    "floating-action-dock-panel pointer-events-auto inline-block w-fit min-w-[16rem] max-w-[calc(100vw-2rem)] rounded-[1.3rem] border border-white/10 bg-zinc-950/95 p-2 text-white shadow-2xl shadow-black/25 backdrop-blur-xl supports-[backdrop-filter]:bg-zinc-950/88 max-sm:min-w-0",
    props.variant === "actions"
      ? "floating-action-dock-panel--actions"
      : "floating-action-dock-panel--surface",
    props.floatingClass,
  ),
);
const floatingContentClasses = computed(() =>
  cn(
    "floating-action-dock-content flex flex-wrap items-center gap-2.5",
    props.align === "center" ? "justify-center" : "justify-end",
  ),
);

const disconnectIntersectionObserver = () => {
  intersectionObserver?.disconnect();
  intersectionObserver = null;
};

const clearKeepVisibleReleaseTimer = () => {
  if (keepVisibleReleaseTimer === null) return;
  window.clearTimeout(keepVisibleReleaseTimer);
  keepVisibleReleaseTimer = null;
};

const connectIntersectionObserver = () => {
  disconnectIntersectionObserver();

  const element = inlineRef.value;
  if (!element || typeof IntersectionObserver === "undefined") {
    isInlineVisible.value = true;
    return;
  }

  const threshold = Math.min(Math.max(props.visibleThreshold, 0), 1);
  intersectionObserver = new IntersectionObserver(
    ([entry]) => {
      if (!entry) return;
      isInlineVisible.value =
        entry.isIntersecting && entry.intersectionRatio >= threshold;
    },
    { threshold: [0, threshold, 1] },
  );
  intersectionObserver.observe(element);
};

const reconnectObservers = async () => {
  await nextTick();
  connectIntersectionObserver();
};

const handleFloatingFocusIn = () => {
  hasFloatingFocus.value = true;
};

const handleFloatingFocusOut = (event: FocusEvent) => {
  const nextTarget = event.relatedTarget;
  if (nextTarget instanceof Node && floatingRef.value?.contains(nextTarget)) {
    return;
  }

  window.setTimeout(() => {
    const activeElement = document.activeElement;
    hasFloatingFocus.value = Boolean(
      activeElement && floatingRef.value?.contains(activeElement),
    );
  }, 0);
};

onMounted(() => {
  void reconnectObservers();
});

onActivated(() => {
  if (isLifecycleActive.value) return;

  isLifecycleActive.value = true;
  void reconnectObservers();
});

onDeactivated(() => {
  isLifecycleActive.value = false;
  disconnectIntersectionObserver();
  clearKeepVisibleReleaseTimer();
  isKeepVisibleReleasing.value = false;
  hasFloatingFocus.value = false;
  isInlineVisible.value = true;
});

onBeforeUnmount(() => {
  isLifecycleActive.value = false;
  disconnectIntersectionObserver();
  clearKeepVisibleReleaseTimer();
});

watch(
  () => props.visibleThreshold,
  () => {
    void reconnectObservers();
  },
);

watch(
  () => props.keepVisible,
  (next, previous) => {
    clearKeepVisibleReleaseTimer();

    if (next) {
      isKeepVisibleReleasing.value = false;
      return;
    }

    if (previous && props.active && props.keepVisibleReleaseDelay > 0) {
      isKeepVisibleReleasing.value = true;
      keepVisibleReleaseTimer = window.setTimeout(() => {
        isKeepVisibleReleasing.value = false;
        keepVisibleReleaseTimer = null;
      }, props.keepVisibleReleaseDelay);
      return;
    }

    isKeepVisibleReleasing.value = false;
  },
);

watch(
  () => [props.active, props.keepVisible, props.keepVisibleReleaseDelay],
  () => {
    if (!props.active && !props.keepVisible && !isKeepVisibleReleasing.value) {
      hasFloatingFocus.value = false;
    }
    if (!props.active) {
      clearKeepVisibleReleaseTimer();
      isKeepVisibleReleasing.value = false;
    }
    void reconnectObservers();
  },
  { flush: "post" },
);
</script>

<template>
  <div ref="inlineRef" :class="inlineClasses">
    <slot name="inline" />
  </div>

  <Teleport to="body">
    <Transition name="floating-action-dock">
      <div v-if="shouldRenderFloating" :class="floatingFrameClasses">
        <div
          ref="floatingRef"
          role="region"
          :aria-label="ariaLabel"
          :class="floatingPanelClasses"
          @focusin="handleFloatingFocusIn"
          @focusout="handleFloatingFocusOut"
        >
          <div :class="floatingContentClasses">
            <slot v-if="slots.floating" name="floating" />
            <slot v-else name="inline" />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.floating-action-dock-enter-active {
  animation: floating-action-dock-rise 520ms cubic-bezier(0.2, 0.9, 0.2, 1) both;
}

.floating-action-dock-leave-active {
  transition:
    opacity 140ms ease-in,
    transform 180ms cubic-bezier(0.4, 0, 1, 1);
}

.floating-action-dock-leave-to {
  opacity: 0;
  transform: translateY(16px) scale(0.97);
}

.floating-action-dock-panel {
  color-scheme: dark;
  position: relative;
  isolation: isolate;
  overflow: visible;
  background: transparent;
  border-color: transparent;
  box-shadow: none;
}

.floating-action-dock-panel::before {
  content: "";
  position: absolute;
  inset: 0;
  z-index: -1;
  border: 1px solid rgb(255 255 255 / 10%);
  border-radius: inherit;
  background: rgb(9 9 11 / 95%);
  box-shadow:
    0 1px 0 rgb(255 255 255 / 8%) inset,
    0 24px 48px -16px rgb(0 0 0 / 42%),
    0 10px 24px -18px rgb(0 0 0 / 72%);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}

.floating-action-dock-enter-active .floating-action-dock-panel::before {
  animation: floating-action-dock-background-in 520ms
    cubic-bezier(0.2, 0.9, 0.2, 1) both;
}

.floating-action-dock-enter-active
  .floating-action-dock-panel
  .floating-action-dock-content {
  animation: floating-action-dock-content-reveal 520ms ease-out both;
}

.floating-action-dock-panel--actions :deep([data-slot="button"]) {
  min-width: 5.65rem;
  min-height: 2.55rem;
  border-color: transparent;
  border-radius: 0.95rem;
  background: rgb(255 255 255 / 12%);
  color: rgb(255 255 255 / 88%);
  box-shadow: none;
  padding-inline: 1.15rem;
  font-size: 0.94rem;
  font-weight: 650;
}

.floating-action-dock-panel--actions :deep([data-slot="button"]:hover) {
  background: rgb(255 255 255 / 18%);
  color: #fff;
}

.floating-action-dock-panel--actions :deep([data-slot="button"]:last-child) {
  background: #fff;
  color: #09090b;
  box-shadow:
    0 1px 0 rgb(255 255 255 / 38%) inset,
    0 10px 24px rgb(0 0 0 / 18%);
}

.floating-action-dock-panel--actions
  :deep([data-slot="button"]:last-child:hover) {
  background: rgb(255 255 255 / 92%);
  color: #09090b;
}

.floating-action-dock-panel--actions :deep([data-slot="button"]:disabled) {
  opacity: 0.55;
}

@keyframes floating-action-dock-rise {
  0% {
    opacity: 0;
    transform: translateY(2rem) scale(0.98);
  }
  24% {
    opacity: 1;
    transform: translateY(-0.28rem) scale(1.01);
  }
  52% {
    opacity: 1;
    transform: translateY(0.08rem) scale(0.998);
  }
  74% {
    transform: translateY(-0.03rem) scale(1);
  }
  100% {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

@keyframes floating-action-dock-background-in {
  0% {
    opacity: 0;
    transform: scaleX(0.18) scaleY(0.72);
    transform-origin: 50% 100%;
  }
  16% {
    opacity: 1;
    transform: scaleX(0.2) scaleY(0.76);
  }
  58% {
    opacity: 1;
    transform: scaleX(1.035) scaleY(1);
  }
  78% {
    transform: scaleX(0.985) scaleY(1);
  }
  100% {
    opacity: 1;
    transform: scaleX(1) scaleY(1);
  }
}

@keyframes floating-action-dock-content-reveal {
  0%,
  48% {
    opacity: 0;
    transform: translateY(0.3rem) scale(0.98);
  }
  76% {
    opacity: 0.72;
  }
  100% {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .floating-action-dock-enter-active {
    animation: none;
  }

  .floating-action-dock-enter-active .floating-action-dock-panel::before,
  .floating-action-dock-enter-active
    .floating-action-dock-panel
    .floating-action-dock-content {
    animation: none;
  }

  .floating-action-dock-leave-active {
    transition: none;
  }
}
</style>
