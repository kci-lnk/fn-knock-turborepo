<script setup lang="ts">
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { toast } from '@admin-shared/utils/toast'
import CodeMirrorEditor, { type CodeEditorLanguage } from './CodeMirrorEditor.vue'

const props = defineProps<{
  modelValue: string
  contentType: string
}>()

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
}>()

const textEncoder = new TextEncoder()

const mediaType = computed(() =>
  props.contentType.split(';')[0]?.trim().toLowerCase() ?? '',
)

const editorLanguage = computed<CodeEditorLanguage>(() => {
  const type = mediaType.value
  if (type === 'application/json' || type === 'text/json' || type.endsWith('+json')) {
    return 'json'
  }
  if (type === 'text/html' || type === 'application/xhtml+xml') {
    return 'html'
  }
  if (type === 'text/css') {
    return 'css'
  }
  if (type === 'application/xml' || type === 'text/xml' || type.endsWith('+xml')) {
    return 'xml'
  }
  if (
    type === 'application/javascript' ||
    type === 'text/javascript' ||
    type === 'application/ecmascript' ||
    type === 'text/ecmascript' ||
    type === 'application/x-javascript'
  ) {
    return 'javascript'
  }
  return 'text'
})

const languageLabel = computed(() => {
  switch (editorLanguage.value) {
    case 'json':
      return 'JSON'
    case 'html':
      return 'HTML'
    case 'css':
      return 'CSS'
    case 'xml':
      return 'XML'
    case 'javascript':
      return 'JavaScript'
    default:
      return 'Text'
  }
})

const lineCount = computed(() => {
  if (!props.modelValue) return 1
  return props.modelValue.split(/\r\n|\r|\n/).length
})

const charCount = computed(() => Array.from(props.modelValue).length)
const byteCount = computed(() => textEncoder.encode(props.modelValue).length)
const byteCountLabel = computed(() => {
  if (byteCount.value < 1024) return `${byteCount.value} B`
  if (byteCount.value < 1024 * 1024) {
    return `${(byteCount.value / 1024).toFixed(1)} KB`
  }
  return `${(byteCount.value / 1024 / 1024).toFixed(1)} MB`
})

const isJsonLanguage = computed(() => editorLanguage.value === 'json')
const jsonWarning = computed(() => {
  if (!isJsonLanguage.value || !props.modelValue.trim()) return ''
  try {
    JSON.parse(props.modelValue)
    return ''
  } catch (error) {
    return error instanceof Error ? error.message : 'JSON 语法有误'
  }
})

function formatJson() {
  try {
    const parsed = JSON.parse(props.modelValue)
    emit('update:modelValue', `${JSON.stringify(parsed, null, 2)}\n`)
  } catch (error) {
    toast.error('JSON 格式化失败', {
      description: error instanceof Error ? error.message : '当前 Body 不是合法 JSON',
    })
  }
}
</script>

<template>
  <div
    class="overflow-hidden rounded-md border border-border bg-background transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/20"
  >
    <div
      class="flex flex-col gap-3 border-b border-border bg-muted/25 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="flex min-w-0 items-center gap-2">
        <Label class="shrink-0 text-sm font-medium">Body</Label>
        <Badge variant="secondary" class="font-mono text-[11px]">
          {{ languageLabel }}
        </Badge>
      </div>

      <div
        class="flex flex-wrap items-center gap-x-3 gap-y-2 text-xs text-muted-foreground"
      >
        <span>{{ lineCount }} 行</span>
        <span>{{ charCount }} 字符</span>
        <span>UTF-8 {{ byteCountLabel }}</span>
        <Button
          v-if="isJsonLanguage"
          type="button"
          variant="outline"
          size="sm"
          class="h-8"
          @click="formatJson"
        >
          格式化 JSON
        </Button>
      </div>
    </div>

    <CodeMirrorEditor
      :model-value="modelValue"
      :language="editorLanguage"
      min-height="280px"
      aria-label="响应 Body"
      flush
      @update:model-value="(value) => emit('update:modelValue', value)"
    />

    <div
      v-if="jsonWarning"
      class="border-t border-amber-200 bg-amber-50 px-4 py-2 text-xs leading-5 text-amber-800"
    >
      JSON 语法警告：{{ jsonWarning }}
    </div>
  </div>
</template>
