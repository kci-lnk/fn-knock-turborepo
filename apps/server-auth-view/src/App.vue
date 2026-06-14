<template>
  <div class="theme-grid-background absolute inset-0 -z-10 h-full w-full">
  </div>
  <div
    class="fixed right-[calc(env(safe-area-inset-right)+1rem)] top-[calc(env(safe-area-inset-top)+1rem)] z-30"
  >
    <ThemeModeToggle />
  </div>
  <RouterView />
</template>

<script setup lang="ts">
import { ThemeModeToggle } from '@/components/ui/theme-toggle'
</script>
