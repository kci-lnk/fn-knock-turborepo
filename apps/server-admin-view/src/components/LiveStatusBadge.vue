<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

interface Props {
  active: boolean
  activeLabel?: string
  inactiveLabel?: string
  pulse?: boolean
  size?: 'xs' | 'sm' | 'md'
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  pulse: true,
  size: 'sm',
  class: '',
})

const { t } = useI18n()
const label = computed(() =>
  props.active
    ? props.activeLabel ?? t('common.active')
    : props.inactiveLabel ?? t('common.inactive'),
)
const dotClass = computed(() =>
  props.active ? 'bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.14)]' : 'bg-zinc-300',
)
const sizeClass = computed(() => {
  if (props.size === 'xs') return 'h-[6px] w-[6px]'
  if (props.size === 'md') return 'h-2.5 w-2.5'
  return 'h-2 w-2'
})
</script>

<template>
  <span
    :aria-label="label"
    :title="label"
    :class="['relative inline-flex shrink-0 align-middle', sizeClass, props.class]"
    role="status"
  >
    <span
      v-if="active && pulse"
      class="absolute inset-0 rounded-full bg-emerald-400/80 animate-ping"
      aria-hidden="true"
    />
    <span
      :class="['relative inline-flex rounded-full', sizeClass, dotClass]"
      aria-hidden="true"
    />
  </span>
</template>
