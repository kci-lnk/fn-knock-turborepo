<template>
  <div class="auth-safe-shell auth-visual-shell flex flex-col">
    <div class="flex flex-1 items-center justify-center">
      <slot />
    </div>
    <slot name="footer" />
  </div>
</template>
