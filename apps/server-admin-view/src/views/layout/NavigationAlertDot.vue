<script setup lang="ts">
defineProps<{ label?: string }>();
</script>

<template>
  <span v-if="label" class="inline-flex shrink-0">
    <span
      class="h-2 w-2 rounded-full bg-red-500 ring-2 ring-background"
      aria-hidden="true"
    />
    <span class="sr-only">{{ label }}</span>
  </span>
</template>
