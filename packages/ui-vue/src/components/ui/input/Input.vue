<script setup lang="ts">
import { computed, useAttrs, type HTMLAttributes } from "vue"
import { useVModel } from "@vueuse/core"
import { cn } from "@/lib/utils"

defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(defineProps<{
  allowPasswordManager?: boolean
  defaultValue?: string | number
  modelValue?: string | number
  class?: HTMLAttributes["class"]
}>(), {
  allowPasswordManager: false,
})

const emits = defineEmits<{
  (e: "update:modelValue", payload: string | number): void
}>()

const passiveModelValue = useVModel(props, "modelValue", emits, {
  passive: true,
  defaultValue: props.defaultValue,
  shouldEmit: () => !props.allowPasswordManager,
})

const modelValue = computed({
  get: () => passiveModelValue.value,
  set: (value: string | number) => {
    passiveModelValue.value = value
    // Autofill can submit in the same event turn, before passive watchers run.
    if (props.allowPasswordManager) {
      emits("update:modelValue", value)
    }
  },
})

const attrs = useAttrs()

const resolvedAutocomplete = computed(() => {
  const autocomplete = attrs.autocomplete
  if (typeof autocomplete === "string" && autocomplete.length > 0) {
    return autocomplete
  }

  return attrs.type === "password" ? "new-password" : "off"
})
</script>

<template>
  <input
    v-model="modelValue"
    v-bind="attrs"
    data-slot="input"
    :autocomplete="resolvedAutocomplete"
    autocapitalize="off"
    autocorrect="off"
    :spellcheck="false"
    :data-form-type="props.allowPasswordManager ? null : 'other'"
    :data-1p-ignore="props.allowPasswordManager ? null : 'true'"
    :data-lpignore="props.allowPasswordManager ? null : 'true'"
    :data-bwignore="props.allowPasswordManager ? null : 'true'"
    :class="cn(
      'file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm',
      'focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]',
      'aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive',
      props.class,
    )"
  >
</template>
