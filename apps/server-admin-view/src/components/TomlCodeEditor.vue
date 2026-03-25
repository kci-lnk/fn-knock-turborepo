<script setup lang="ts">
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { defaultHighlightStyle, StreamLanguage, syntaxHighlighting } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from '@codemirror/view'
import { toml } from '@codemirror/legacy-modes/mode/toml'
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'

interface Props {
  modelValue: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
}>()

const hostRef = ref<HTMLDivElement | null>(null)
const editorView = shallowRef<EditorView | null>(null)

const tomlLanguage = StreamLanguage.define(toml)

function buildEditorState(doc: string) {
  return EditorState.create({
    doc,
    extensions: [
      lineNumbers(),
      history(),
      drawSelection(),
      highlightActiveLineGutter(),
      highlightActiveLine(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      tomlLanguage,
      keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
      EditorView.lineWrapping,
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return
        emit('update:modelValue', update.state.doc.toString())
      }),
    ],
  })
}

onMounted(() => {
  if (!hostRef.value) return
  editorView.value = new EditorView({
    parent: hostRef.value,
    state: buildEditorState(props.modelValue),
  })
})

watch(
  () => props.modelValue,
  (value) => {
    const view = editorView.value
    if (!view) return
    const current = view.state.doc.toString()
    if (current === value) return
    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: value,
      },
    })
  },
)

onBeforeUnmount(() => {
  editorView.value?.destroy()
  editorView.value = null
})
</script>

<template>
  <div class="toml-editor-shell">
    <div ref="hostRef" class="toml-editor-host" />
  </div>
</template>

<style scoped>
.toml-editor-shell {
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: calc(var(--radius) + 0.125rem);
  background:
    linear-gradient(
      180deg,
      color-mix(in oklab, var(--color-muted) 38%, var(--color-card)) 0%,
      var(--color-card) 100%
    );
  box-shadow:
    inset 0 1px 0 color-mix(in oklab, white 60%, transparent);
}

.toml-editor-shell:focus-within {
  box-shadow:
    inset 0 1px 0 color-mix(in oklab, white 60%, transparent);
}

.toml-editor-host {
  min-height: 340px;
}

.toml-editor-shell :deep(.cm-editor) {
  height: 100%;
  background: transparent;
  color: var(--color-foreground);
  font-size: 13px;
}

.toml-editor-shell :deep(.cm-scroller) {
  min-height: 340px;
  font-family:
    "SF Mono",
    "Cascadia Code",
    "JetBrains Mono",
    ui-monospace,
    SFMono-Regular,
    Menlo,
    Monaco,
    Consolas,
    "Liberation Mono",
    "Courier New",
    monospace;
  line-height: 1.65;
}

.toml-editor-shell :deep(.cm-content) {
  padding: 14px 0 18px;
  caret-color: var(--color-foreground);
}

.toml-editor-shell :deep(.cm-line) {
  padding: 0 16px;
}

.toml-editor-shell :deep(.cm-gutters) {
  min-height: 340px;
  border-right: 1px solid color-mix(in oklab, var(--color-border) 85%, transparent);
  background: color-mix(in oklab, var(--color-muted) 66%, var(--color-card));
  color: var(--color-muted-foreground);
}

.toml-editor-shell :deep(.cm-activeLine) {
  background: color-mix(in oklab, var(--color-muted) 46%, transparent);
}

.toml-editor-shell :deep(.cm-activeLineGutter) {
  background: color-mix(in oklab, var(--color-muted) 76%, var(--color-card));
  color: var(--color-foreground);
}

.toml-editor-shell :deep(.cm-selectionBackground),
.toml-editor-shell :deep(.cm-content ::selection) {
  background: color-mix(in oklab, var(--color-primary) 26%, white 74%);
}
</style>
