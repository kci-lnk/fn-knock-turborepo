<template>
  <div class="auth-safe-shell auth-visual-shell flex flex-col">
    <main class="flex flex-1 items-center justify-center">
      <slot />
    </main>
    <slot name="footer" />
  </div>
</template>
