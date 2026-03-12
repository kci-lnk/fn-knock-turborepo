<script setup lang="ts">
import { computed } from 'vue'

export interface LogViewerProps {
  /** 日志行（字符串数组或任意对象数组） */
  logs?: unknown[]
  /** 标题，默认"运行日志" */
  title?: string
  /** 是否倒序显示（newest first），默认 false */
  reversed?: boolean
  /** 空状态提示，默认"暂无日志" */
  emptyText?: string
  /** 容器高度 class，默认 "h-72" */
  heightClass?: string
  /** 是否换行（whitespace-pre-wrap vs whitespace-pre），默认 false */
  wrap?: boolean
  /** 是否显示标题栏，默认 true */
  showHeader?: boolean
  /** 主题：dark（终端风格黑底绿字）或 light（白底暗字），默认 dark */
  theme?: 'dark' | 'light'
}

const props = withDefaults(defineProps<LogViewerProps>(), {
  logs: () => [],
  title: '运行日志',
  reversed: false,
  emptyText: '暂无日志',
  heightClass: 'h-72',
  wrap: false,
  showHeader: true,
  theme: 'light',
})

const displayLogs = computed(() =>
  props.reversed ? [...props.logs].reverse() : props.logs,
)

const isDark = computed(() => props.theme === 'dark')
</script>

<template>
  <div
    class="overflow-hidden rounded-lg border"
    :class="isDark ? 'bg-black/90' : 'bg-background'"
  >
    <div
      v-if="showHeader"
      class="flex items-center justify-between gap-2 border-b px-3 py-2"
      :class="isDark ? 'border-white/10' : 'border-border'"
    >
      <div class="text-xs font-medium" :class="isDark ? 'text-white/80' : 'text-foreground'">{{ title }}</div>
      <div class="text-xs" :class="isDark ? 'text-white/40' : 'text-muted-foreground'">{{ logs.length }} 行</div>
    </div>
    <div
      :class="[
        heightClass,
        'overflow-auto p-3 font-mono text-xs leading-5',
        isDark ? 'text-green-200' : 'text-foreground',
      ]"
    >
      <div v-if="logs.length === 0" :class="isDark ? 'text-white/50' : 'text-muted-foreground'">{{ emptyText }}</div>
      <template v-else>
        <slot :logs="displayLogs">
          <div
            v-for="(line, idx) in displayLogs"
            :key="idx"
            :class="wrap ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'"
          >{{ line }}</div>
        </slot>
      </template>
    </div>
  </div>
</template>
