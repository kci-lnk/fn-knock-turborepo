<script setup lang="ts">
defineProps<{
  fullscreen: boolean;
  fullscreenLabel: string;
  title: string;
  toggleFullscreen: () => void;
}>();
</script>

<template>
  <div
    class="relative flex h-[31px] shrink-0 items-center border-b border-black/28 bg-[linear-gradient(180deg,rgba(58,58,61,0.94)_0%,rgba(45,45,48,0.94)_100%)] px-3 backdrop-blur-xl rounded-t-[18px]"
  >
    <div
      class="pointer-events-none absolute inset-x-0 top-0 h-px bg-white/10"
    />
    <div class="z-10 flex items-center gap-2">
      <span
        class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#ff5f57] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.2)]"
      />
      <span
        class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#febc2e] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.18)]"
      />
      <button
        type="button"
        class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#28c840] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.18)] transition-transform hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#7ee78d]/70"
        :class="fullscreen ? 'ring-1 ring-white/35' : ''"
        :aria-label="fullscreenLabel"
        :aria-pressed="fullscreen"
        :title="fullscreenLabel"
        @click="toggleFullscreen"
      >
        <span class="sr-only">{{ fullscreenLabel }}</span>
      </button>
    </div>

    <div
      class="pointer-events-none absolute left-1/2 top-1/2 min-w-0 max-w-[56%] -translate-x-1/2 -translate-y-1/2 text-center"
    >
      <p
        class="truncate text-[12px] font-medium tracking-[-0.01em] text-white/68"
      >
        {{ title }}
      </p>
    </div>
  </div>
</template>
