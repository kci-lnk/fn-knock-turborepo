<script setup lang="ts">
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import {
  defaultHighlightStyle,
  StreamLanguage,
  syntaxHighlighting,
} from "@codemirror/language";
import { css } from "@codemirror/legacy-modes/mode/css";
import { javascript, json } from "@codemirror/legacy-modes/mode/javascript";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { html, xml } from "@codemirror/legacy-modes/mode/xml";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
  type HTMLAttributes,
} from "vue";
import { useI18n } from "vue-i18n";
import { cn } from "@/lib/utils";

export type CodeEditorLanguage =
  | "text"
  | "toml"
  | "json"
  | "html"
  | "css"
  | "xml"
  | "javascript";

interface Props {
  modelValue: string;
  language?: CodeEditorLanguage;
  minHeight?: string;
  ariaLabel?: string;
  flush?: boolean;
  class?: HTMLAttributes["class"];
}

const props = withDefaults(defineProps<Props>(), {
  language: "text",
  minHeight: "260px",
});

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

const hostRef = ref<HTMLDivElement | null>(null);
const editorView = shallowRef<EditorView | null>(null);
const languageCompartment = new Compartment();
const { t } = useI18n();

const languageExtensions: Record<
  Exclude<CodeEditorLanguage, "text">,
  Extension
> = {
  toml: StreamLanguage.define(toml),
  json: StreamLanguage.define(json),
  html: StreamLanguage.define(html),
  css: StreamLanguage.define(css),
  xml: StreamLanguage.define(xml),
  javascript: StreamLanguage.define(javascript),
};

const shellClass = computed(() =>
  cn(
    "code-editor-shell",
    props.flush && "code-editor-shell--flush",
    props.class,
  ),
);
const shellStyle = computed(() => ({
  "--code-editor-min-height": props.minHeight,
}));
const resolvedAriaLabel = computed(
  () => props.ariaLabel || t("admin.components.codeMirrorEditor.ariaLabel"),
);

function getLanguageExtension(language: CodeEditorLanguage): Extension {
  if (language === "text") return [];
  return languageExtensions[language] ?? [];
}

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
      languageCompartment.of(getLanguageExtension(props.language)),
      keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({
        "aria-label": resolvedAriaLabel.value,
      }),
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return;
        emit("update:modelValue", update.state.doc.toString());
      }),
    ],
  });
}

onMounted(() => {
  if (!hostRef.value) return;
  editorView.value = new EditorView({
    parent: hostRef.value,
    state: buildEditorState(props.modelValue),
  });
});

watch(
  () => props.modelValue,
  (value) => {
    const view = editorView.value;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === value) return;
    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: value,
      },
    });
  },
);

watch(
  () => props.language,
  (language) => {
    const view = editorView.value;
    if (!view) return;
    view.dispatch({
      effects: languageCompartment.reconfigure(getLanguageExtension(language)),
    });
  },
);

onBeforeUnmount(() => {
  editorView.value?.destroy();
  editorView.value = null;
});

defineExpose({
  insertText(value: string) {
    const view = editorView.value;
    if (!view) return;
    const selection = view.state.selection.main;
    view.dispatch({
      changes: { from: selection.from, to: selection.to, insert: value },
      selection: { anchor: selection.from + value.length },
    });
    view.focus();
  },
});
</script>

<template>
  <div :class="shellClass" :style="shellStyle">
    <div ref="hostRef" class="code-editor-host" />
  </div>
</template>

<style scoped>
.code-editor-shell {
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: calc(var(--radius) + 0.125rem);
  background: linear-gradient(
    180deg,
    color-mix(in oklab, var(--color-muted) 38%, var(--color-card)) 0%,
    var(--color-card) 100%
  );
  box-shadow: inset 0 1px 0 color-mix(in oklab, white 60%, transparent);
  transition:
    border-color 150ms ease,
    box-shadow 150ms ease;
}

.code-editor-shell:focus-within {
  border-color: color-mix(in oklab, var(--color-ring) 70%, var(--color-border));
  box-shadow:
    0 0 0 3px color-mix(in oklab, var(--color-ring) 18%, transparent),
    inset 0 1px 0 color-mix(in oklab, white 60%, transparent);
}

.code-editor-shell.code-editor-shell--flush {
  border: 0;
  border-radius: 0;
  box-shadow: none;
}

.code-editor-shell.code-editor-shell--flush:focus-within {
  border-color: transparent;
  box-shadow: none;
}

.code-editor-host {
  min-height: var(--code-editor-min-height);
}

.code-editor-shell :deep(.cm-editor) {
  height: 100%;
  background: transparent;
  color: var(--color-foreground);
  font-size: 13px;
}

.code-editor-shell :deep(.cm-scroller) {
  min-height: var(--code-editor-min-height);
  font-family:
    "SF Mono", "Cascadia Code", "JetBrains Mono", ui-monospace, SFMono-Regular,
    Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  line-height: 1.65;
}

.code-editor-shell :deep(.cm-content) {
  padding: 14px 0 18px;
  caret-color: var(--color-foreground);
}

.code-editor-shell :deep(.cm-line) {
  padding: 0 16px;
}

.code-editor-shell :deep(.cm-gutters) {
  min-height: var(--code-editor-min-height);
  border-right: 1px solid
    color-mix(in oklab, var(--color-border) 85%, transparent);
  background: color-mix(in oklab, var(--color-muted) 66%, var(--color-card));
  color: var(--color-muted-foreground);
}

.code-editor-shell :deep(.cm-activeLine) {
  background: color-mix(in oklab, var(--color-muted) 46%, transparent);
}

.code-editor-shell :deep(.cm-activeLineGutter) {
  background: color-mix(in oklab, var(--color-muted) 76%, var(--color-card));
  color: var(--color-foreground);
}

.code-editor-shell :deep(.cm-selectionBackground),
.code-editor-shell :deep(.cm-content ::selection) {
  background: color-mix(in oklab, var(--color-primary) 26%, white 74%);
}
</style>
