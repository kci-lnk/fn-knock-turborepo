<script setup lang="ts">
withDefaults(defineProps<{ controlClass?: string }>(), { controlClass: "" });
</script>

<template>
  <div
    class="grid gap-4 p-6 md:grid-cols-[320px_minmax(0,1fr)] md:items-start md:gap-x-8"
  >
    <div class="min-w-0 space-y-1 md:pt-1">
      <slot name="copy" />
    </div>
    <div
      :class="[
        'w-full md:mt-3.5 md:w-[88%] md:justify-self-end',
        controlClass,
      ]"
    >
      <slot />
    </div>
  </div>
</template>
