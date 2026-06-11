<script setup lang="ts">
import CodeMirrorEditor from './CodeMirrorEditor.vue'

interface Props {
  modelValue: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
}>()
</script>

<template>
  <CodeMirrorEditor
    :model-value="props.modelValue"
    language="toml"
    min-height="340px"
    aria-label="frpc.toml"
    @update:model-value="(value) => emit('update:modelValue', value)"
  />
</template>
