<script setup lang="ts">
import { ref, type CSSProperties } from "vue";
import { useI18n } from "vue-i18n";
import { ClipboardPaste, Copy, TextSelect } from "lucide-vue-next";

defineProps<{
  canPaste: boolean;
  hasSelection: boolean;
  menuStyle: CSSProperties;
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
  copy: [];
  paste: [];
  selectAll: [];
}>();

const { t } = useI18n();
const rootElement = ref<HTMLElement | null>(null);

const handleFocusOut = (event: FocusEvent) => {
  const nextTarget = event.relatedTarget;
  if (nextTarget instanceof Node && rootElement.value?.contains(nextTarget)) {
    return;
  }
  emit("close");
};

defineExpose({
  rootElement,
});
</script>

<template>
  <div
    v-if="open"
    ref="rootElement"
    :style="menuStyle"
    class="fixed z-[70] w-44 overflow-hidden rounded-lg border border-white/12 bg-[#29292d]/95 p-1 text-sm text-white shadow-[0_16px_44px_rgba(0,0,0,0.38)] outline-none backdrop-blur-xl"
    role="group"
    :aria-label="t('admin.webTerminal.contextMenu')"
    tabindex="-1"
    @contextmenu.prevent.stop
    @pointerdown.stop
    @click.stop
    @focusout="handleFocusOut"
  >
    <button
      type="button"
      class="flex h-9 w-full items-center gap-2 rounded-md px-2.5 text-left transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:text-white/35 disabled:hover:bg-transparent"
      :disabled="!hasSelection"
      @click="emit('copy')"
    >
      <Copy class="h-4 w-4" />
      <span>{{ t("admin.webTerminal.copy") }}</span>
    </button>
    <button
      type="button"
      class="flex h-9 w-full items-center gap-2 rounded-md px-2.5 text-left transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:text-white/35 disabled:hover:bg-transparent"
      :disabled="!canPaste"
      @click="emit('paste')"
    >
      <ClipboardPaste class="h-4 w-4" />
      <span>{{ t("admin.webTerminal.paste") }}</span>
    </button>
    <button
      type="button"
      class="flex h-9 w-full items-center gap-2 rounded-md px-2.5 text-left transition-colors hover:bg-white/10"
      @click="emit('selectAll')"
    >
      <TextSelect class="h-4 w-4" />
      <span>{{ t("admin.webTerminal.selectAll") }}</span>
    </button>
  </div>
</template>
