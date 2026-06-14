<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { computed } from "vue"
import { Moon, Sun } from "lucide-vue-next"
import { useI18n } from "vue-i18n"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { useThemeMode } from "./useThemeMode"

const props = defineProps<{
  buttonClass?: HTMLAttributes["class"]
}>()

const { t } = useI18n()
const { mode, toggleThemeMode } = useThemeMode()

const isDark = computed(() => mode.value === "dark")

const currentIcon = computed(() => (isDark.value ? Moon : Sun))

const buttonLabel = computed(() =>
  isDark.value
    ? t("common.switchToLightAppearance")
    : t("common.switchToDarkAppearance"),
)

const handleClick = (event: MouseEvent) => {
  void toggleThemeMode(event)
}
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    :class="
      cn(
        'h-8 w-8 rounded-md border border-border/60 bg-background/70 text-muted-foreground shadow-none transition-[background-color,border-color,color,transform] duration-200 hover:bg-muted hover:text-foreground hover:-translate-y-px',
        props.buttonClass,
      )
    "
    :aria-label="buttonLabel"
    :title="buttonLabel"
    @click="handleClick"
  >
    <component
      :is="currentIcon"
      class="h-4 w-4 transition-[transform,opacity] duration-200"
    />
    <span class="sr-only">{{ buttonLabel }}</span>
  </Button>
</template>
