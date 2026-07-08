<script setup lang="ts">
import { Label } from "@/components/ui/label";

withDefaults(
  defineProps<{
    contentClass?: string;
    hint?: string;
    id: string;
    label: string;
    mobileHint?: string;
    required?: boolean;
  }>(),
  {
    contentClass: "max-w-md",
    hint: "",
    mobileHint: "",
    required: false,
  },
);
</script>

<template>
  <div
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label :for="id" class="flex items-center gap-1 text-sm font-medium">
        {{ label }}
        <span v-if="required" class="text-destructive">*</span>
      </Label>
      <p
        v-if="hint"
        class="hidden pr-4 text-xs leading-relaxed text-muted-foreground sm:block"
      >
        {{ hint }}
      </p>
    </div>
    <div
      :class="['w-full', contentClass, hint || mobileHint ? 'space-y-2' : '']"
    >
      <slot />
      <p
        v-if="mobileHint || hint"
        class="mt-1.5 text-[11px] text-muted-foreground sm:hidden"
      >
        {{ mobileHint || hint }}
      </p>
    </div>
  </div>
</template>
