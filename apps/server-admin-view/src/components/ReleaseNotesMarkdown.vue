<script setup lang="ts">
import { computed } from "vue";
import { renderReleaseNotesHtml } from "../lib/release-notes";

const props = defineProps<{
  source: string | null | undefined;
  fallback: string;
}>();

const html = computed(() =>
  renderReleaseNotesHtml(props.source, props.fallback),
);
</script>

<template>
  <div
    class="release-notes-markdown break-words text-sm leading-relaxed text-muted-foreground"
    v-html="html"
  ></div>
</template>

<style scoped>
.release-notes-markdown :deep(> :first-child) {
  margin-top: 0;
}

.release-notes-markdown :deep(> :last-child) {
  margin-bottom: 0;
}

.release-notes-markdown :deep(h4) {
  margin-block: 1.25rem 0.625rem;
  color: var(--foreground);
  font-weight: 650;
  line-height: 1.4;
  font-size: 1.125rem;
}

.release-notes-markdown :deep(p) {
  margin-block: 0.75rem;
}

.release-notes-markdown :deep(.release-note-alert) {
  --release-note-alert-color: #0969da;

  margin-block: 1rem;
  padding: 0.8rem 0.9rem;
  border-left: 0.25rem solid var(--release-note-alert-color);
  border-radius: 0.35rem;
  background: color-mix(
    in oklch,
    var(--release-note-alert-color) 9%,
    transparent
  );
}

.release-notes-markdown :deep(.release-note-alert--tip) {
  --release-note-alert-color: #1a7f37;
}

.release-notes-markdown :deep(.release-note-alert--important) {
  --release-note-alert-color: #8250df;
}

.release-notes-markdown :deep(.release-note-alert--warning) {
  --release-note-alert-color: #9a6700;
}

.release-notes-markdown :deep(.release-note-alert--caution) {
  --release-note-alert-color: #d1242f;
}

.release-notes-markdown :deep(.release-note-alert__title) {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin-block: 0 0.4rem;
  color: var(--release-note-alert-color);
  font-weight: 650;
}

.release-notes-markdown :deep(.release-note-alert__title::before) {
  display: inline-grid;
  width: 1rem;
  height: 1rem;
  place-items: center;
  border: 1.5px solid currentcolor;
  border-radius: 999px;
  content: "!";
  font-size: 0.68rem;
  line-height: 1;
}

.release-notes-markdown :deep(.release-note-alert__body > :first-child) {
  margin-top: 0;
}

.release-notes-markdown :deep(.release-note-alert__body > :last-child) {
  margin-bottom: 0;
}

.release-notes-markdown :deep(ul) {
  margin-block: 0.75rem;
  padding-inline-start: 1.4rem;
  list-style: disc;
}

.release-notes-markdown :deep(li) {
  margin-block: 0.35rem;
  padding-inline-start: 0.15rem;
}

.release-notes-markdown :deep(li::marker) {
  color: var(--muted-foreground);
}

.release-notes-markdown :deep(strong) {
  color: var(--foreground);
  font-weight: 650;
}

.release-notes-markdown :deep(a) {
  color: var(--primary);
  font-weight: 550;
  text-decoration: underline;
  text-decoration-color: color-mix(in oklch, var(--primary) 50%, transparent);
  text-underline-offset: 0.2em;
}

.release-notes-markdown :deep(a:hover) {
  text-decoration-color: var(--primary);
}

.release-notes-markdown :deep(hr) {
  margin-block: 1.25rem;
  border: 0;
  border-top: 1px solid var(--border);
}
</style>
