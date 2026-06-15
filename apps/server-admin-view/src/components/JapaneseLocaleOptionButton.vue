<template>
  <button
    type="button"
    :class="[
      'group flex h-14 w-full items-center gap-3 px-5 text-left transition-colors',
      selected ? 'bg-muted/90' : 'hover:bg-muted/55',
      disabled ? 'cursor-not-allowed opacity-60' : '',
    ]"
    :disabled="disabled"
    :aria-current="selected ? 'true' : undefined"
    @click="handleSelect"
    @pointerenter="triggerHover(true)"
    @pointerleave="triggerHover(false)"
    @focus="triggerHover(true)"
    @blur="triggerHover(false)"
  >
    <span
      :class="[
        'grid h-5 w-5 shrink-0 place-items-center rounded-full border transition-colors',
        selected
          ? 'border-emerald-500 bg-emerald-500 text-white'
          : 'border-muted-foreground/35',
      ]"
    >
      <Check v-if="selected" class="h-3.5 w-3.5" />
    </span>

    <span class="min-w-0 flex-1 truncate text-sm font-medium">
      {{ label }}
    </span>

    <span class="jp-flag-stage" aria-hidden="true">
      
      <span 
        class="jp-flag-glow" 
        :class="{ 'is-active': isHovered || selected, 'is-surge': isSurging }" 
      />

      <svg
        class="ribbon-svg ribbon-back"
        :class="{ 'is-flowing': isHovered || selected, 'is-surge': isSurging }"
        viewBox="0 0 64 48"
        fill="none"
      >
        <defs>
          <linearGradient id="fifa-grad-back" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#00529b" />
            <stop offset="50%" stop-color="#00aeef" />
            <stop offset="100%" stop-color="#00a651" />
          </linearGradient>
        </defs>
        <path d="M -4,12 Q 32,-4 68,14" stroke="url(#fifa-grad-back)" stroke-width="3" stroke-linecap="round" />
        <path d="M 68,34 Q 32,52 -4,36" stroke="url(#fifa-grad-back)" stroke-width="3" stroke-linecap="round" />
      </svg>

      <span class="jp-flag-card">
        <svg viewBox="0 0 32 24" class="h-6 w-8">
          <rect width="32" height="24" fill="#fff" />
          <circle cx="16" cy="12" r="5.4" fill="#bc002d" />
        </svg>
      </span>

      <svg
        class="ribbon-svg ribbon-front"
        :class="{ 'is-flowing': isHovered || selected, 'is-surge': isSurging }"
        viewBox="0 0 64 48"
        fill="none"
      >
        <defs>
          <linearGradient id="fifa-grad-front" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#ed1c24" />
            <stop offset="50%" stop-color="#fff200" />
            <stop offset="100%" stop-color="#00aeef" />
          </linearGradient>
        </defs>
        <path d="M 68,14 Q 32,30 -4,36" stroke="url(#fifa-grad-front)" stroke-width="3" stroke-linecap="round" />
        <path d="M -4,36 Q 32,18 68,14" stroke="url(#fifa-grad-front)" stroke-width="3" stroke-linecap="round" />
      </svg>

    </span>
  </button>
</template>

<script setup lang="ts">
import { ref, onUnmounted } from "vue";
import { Check } from "lucide-vue-next";

defineProps<{
  label: string;
  selected: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  select: [];
}>();

const isHovered = ref(false);
const isSurging = ref(false);
let surgeTimer: ReturnType<typeof window.setTimeout> | undefined;

const triggerHover = (state: boolean) => {
  isHovered.value = state;
};

const playRibbonSurge = () => {
  if (surgeTimer !== undefined) {
    window.clearTimeout(surgeTimer);
  }
  
  isSurging.value = true;

  surgeTimer = window.setTimeout(() => {
    isSurging.value = false;
    surgeTimer = undefined;
  }, 800);
};

const handleSelect = () => {
  emit("select");
  playRibbonSurge();
};

onUnmounted(() => {
  if (surgeTimer !== undefined) window.clearTimeout(surgeTimer);
});
</script>

<style scoped>
.jp-flag-stage {
  position: relative;
  display: grid;
  width: 2rem;
  height: 1.5rem;
  flex-shrink: 0;
  place-items: center;
  overflow: visible; 
}

.jp-flag-card {
  position: relative;
  z-index: 2; 
  display: grid;
  width: 2rem;
  height: 1.5rem;
  place-items: center;
  overflow: hidden;
  border-radius: 5px;
  background: white;
  box-shadow:
    0 1px 2px rgb(15 23 42 / 0.12),
    0 0 0 1px rgb(0 0 0 / 0.1);
  transition: transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.group:hover .jp-flag-card,
.group:focus-visible .jp-flag-card {
  transform: translateY(-1px) scale(1.05);
}

.jp-flag-glow {
  position: absolute;
  inset: -0.5rem;
  z-index: 0;
  border-radius: 50%;
  background: radial-gradient(
    circle,
    rgba(0, 174, 239, 0.3) 0%,
    rgba(237, 28, 36, 0.15) 45%,
    rgba(0, 166, 81, 0) 70%
  );
  filter: blur(8px);
  opacity: 0;
  transform: scale(0.8);
  transition: opacity 300ms ease, transform 300ms ease;
}

.jp-flag-glow.is-active {
  opacity: 1;
  transform: scale(1.1);
}

.jp-flag-glow.is-surge {
  animation: glow-surge-pulse 0.8s ease-out forwards;
}

@keyframes glow-surge-pulse {
  0% {
    opacity: 1;
    transform: scale(1);
    filter: blur(4px);
    background: radial-gradient(circle, rgba(255, 242, 0, 0.6) 0%, rgba(237, 28, 36, 0.3) 50%, transparent 70%);
  }
  100% {
    opacity: 0;
    transform: scale(1.6);
    filter: blur(12px);
  }
}

.ribbon-svg {
  position: absolute;
  inset: -0.6rem -0.8rem; 
  width: calc(100% + 1.6rem);
  height: calc(100% + 1.2rem);
  pointer-events: none;
  overflow: visible;
}

.ribbon-back {
  z-index: 1; 
}

.ribbon-front {
  z-index: 3; 
}

.ribbon-svg path {
  stroke-dasharray: 40;
  stroke-dashoffset: 40;
  opacity: 0;
  transition: opacity 250ms ease, stroke-width 250ms ease;
}

.ribbon-svg.is-flowing path {
  opacity: 0.9;
  animation: ribbon-flow-run 1.8s linear infinite;
}

.ribbon-svg.is-surge path {
  opacity: 1;
  stroke-width: 4.5px;
  animation: ribbon-flow-run 0.4s linear infinite;
}

@keyframes ribbon-flow-run {
  0% {
    stroke-dashoffset: 80;
  }
  100% {
    stroke-dashoffset: 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .jp-flag-card,
  .jp-flag-glow {
    transition: none;
  }
  .ribbon-svg path {
    animation: none !important;
    stroke-dasharray: none !important;
    opacity: 0.7 !important;
  }
}
</style>