<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import type { ArmedModifier } from "./terminal-runtime";

type ToolbarShortcut = {
  id: string;
  label: string;
  value: string;
};

const props = defineProps<{
  armedModifier: ArmedModifier | null;
  armedModifierLabel: string;
  disabled: boolean;
  fontSize: number;
  keepFocused: (event: Event) => void;
  modifierLabels: Record<ArmedModifier, string>;
  navigationShortcuts: ToolbarShortcut[];
  nudgeFontSize: (delta: number) => void;
  primaryShortcuts: ToolbarShortcut[];
  resetFontSize: () => void;
  sendShortcut: (value: string) => void;
  show: boolean;
  toggleModifier: (modifier: ArmedModifier) => void;
}>();

const { t } = useI18n();

const modifierKeys = computed(
  () => Object.keys(props.modifierLabels) as ArmedModifier[],
);
</script>

<template>
  <div
    class="flex items-center gap-2 overflow-x-auto pb-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
  >
    <template v-if="show">
      <Button
        v-for="item in primaryShortcuts"
        :key="item.id"
        size="sm"
        variant="outline"
        class="h-9 shrink-0 rounded-xl border-border/70 bg-background/80 px-3 shadow-none"
        :disabled="disabled"
        @pointerdown="keepFocused"
        @click="sendShortcut(item.value)"
      >
        {{ item.label }}
      </Button>

      <Button
        v-for="modifier in modifierKeys"
        :key="modifier"
        size="sm"
        variant="outline"
        class="h-9 shrink-0 rounded-xl px-3 shadow-none transition-colors"
        :class="
          armedModifier === modifier
            ? 'border-primary/50 bg-primary/10 text-primary'
            : 'border-border/70 bg-background/80'
        "
        :aria-pressed="armedModifier === modifier"
        :disabled="disabled"
        @pointerdown="keepFocused"
        @click="toggleModifier(modifier)"
      >
        {{ modifierLabels[modifier] }}
      </Button>

      <Button
        v-for="item in navigationShortcuts"
        :key="item.id"
        size="sm"
        variant="outline"
        class="h-9 shrink-0 rounded-xl border-border/70 bg-background/80 px-3 shadow-none"
        :disabled="disabled"
        @pointerdown="keepFocused"
        @click="sendShortcut(item.value)"
      >
        {{ item.label }}
      </Button>

      <div class="h-8 w-px shrink-0 bg-border/70" />

      <div class="flex shrink-0 items-center gap-1.5">
        <Button
          size="sm"
          variant="ghost"
          class="h-9 rounded-xl px-3 text-[13px] font-semibold"
          @pointerdown="keepFocused"
          @click="nudgeFontSize(-1)"
        >
          A-
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="h-9 min-w-[64px] rounded-xl px-3 font-mono text-[12px] text-muted-foreground"
          @pointerdown="keepFocused"
          @click="resetFontSize"
        >
          {{ fontSize }}px
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="h-9 rounded-xl px-3 text-[13px] font-semibold"
          @pointerdown="keepFocused"
          @click="nudgeFontSize(1)"
        >
          A+
        </Button>
      </div>

      <div
        v-if="armedModifier"
        class="shrink-0 rounded-xl border border-primary/35 bg-primary/10 px-3 py-2 text-[11px] font-medium text-primary"
      >
        {{
          t("admin.webTerminal.modifierLocked", {
            modifier: armedModifierLabel,
          })
        }}
      </div>
    </template>
  </div>
</template>
