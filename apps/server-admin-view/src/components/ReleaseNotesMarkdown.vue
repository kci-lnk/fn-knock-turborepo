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
