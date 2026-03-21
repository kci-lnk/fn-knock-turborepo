<script setup lang="ts">
import { ref, useSlots, watch } from 'vue'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'

interface Props {
  title: string
  configured: boolean
  ready?: boolean
  editLabel?: string
  collapsedContentClass?: string
  expandedContentClass?: string
  summaryClass?: string
  actionsClass?: string
}

const props = withDefaults(defineProps<Props>(), {
  ready: true,
  editLabel: '编辑配置',
  collapsedContentClass: 'h-[40px] flex items-center justify-between gap-3',
  expandedContentClass: '',
  summaryClass: 'text-xs text-muted-foreground truncate w-64 max-w-full',
  actionsClass: '',
})

const slots = useSlots()

const open = ref(false)
const initialized = ref(false)
const wrapperRef = ref<HTMLElement | null>(null)

const TRANSITION_DURATION = 250 

function expand() {
  open.value = true
  initialized.value = true
}

function collapse() {
  open.value = false
  initialized.value = true
}

function toggle() {
  open.value = !open.value
  initialized.value = true
}

function onBeforeEnter(element: Element) {
  const el = element as HTMLElement
  el.style.opacity = '0'
}

function onEnter(element: Element, done: () => void) {
  const el = element as HTMLElement
  const wrapper = wrapperRef.value
  if (!wrapper) return done()
  el.style.width = '100%'
  const targetHeight = el.offsetHeight
  void wrapper.offsetHeight

  wrapper.style.overflow = 'hidden'
  wrapper.style.height = `${targetHeight}px`

  el.style.transition = `opacity ${TRANSITION_DURATION}ms ease`
  el.style.opacity = '1'

  setTimeout(done, TRANSITION_DURATION)
}

function onAfterEnter(element: Element) {
  const el = element as HTMLElement
  el.style.opacity = ''
  el.style.transition = ''
  el.style.width = ''

  const wrapper = wrapperRef.value
  if (wrapper) {
    wrapper.style.height = 'auto'
    wrapper.style.overflow = ''
  }
}

function onBeforeLeave() {
  const wrapper = wrapperRef.value
  if (wrapper) {
    wrapper.style.height = `${wrapper.offsetHeight}px`
    wrapper.style.overflow = 'hidden'
  }
}

function onLeave(element: Element, done: () => void) {
  const el = element as HTMLElement
  el.style.position = 'absolute'
  el.style.top = '0'
  el.style.left = '0'
  el.style.width = '100%'

  el.style.transition = `opacity ${TRANSITION_DURATION}ms ease`
  void el.offsetHeight 
  el.style.opacity = '0'

  setTimeout(done, TRANSITION_DURATION)
}

function onAfterLeave(element: Element) {
  const el = element as HTMLElement
  el.style.position = ''
  el.style.top = ''
  el.style.left = ''
  el.style.width = ''
  el.style.opacity = ''
  el.style.transition = ''
}

watch(
  () => props.ready,
  (ready) => {
    if (ready && !initialized.value) {
      open.value = !props.configured
      initialized.value = true
    }
  },
  { immediate: true },
)
</script>

<template>
  <Card :class="open ? 'py-0 gap-0' : ''">
    <div
      ref="wrapperRef"
      class="relative transition-[height] ease-in-out"
      :style="{ transitionDuration: `${TRANSITION_DURATION}ms` }"
    >
      <Transition
        @before-enter="onBeforeEnter"
        @enter="onEnter"
        @after-enter="onAfterEnter"
        @before-leave="onBeforeLeave"
        @leave="onLeave"
        @after-leave="onAfterLeave"
      >
        <div v-if="!open" key="collapsed" class="w-full">
          <CardContent 
            :class="[
              collapsedContentClass, 
              'cursor-pointer transition-colors duration-200 hover:bg-muted/50'
            ]"
            @click="expand"
          >
            <div class="min-w-0 flex-1 space-y-1">
              <div class="text-sm font-medium">{{ title }}</div>
              <div :class="summaryClass">
                <slot name="summary" />
              </div>
            </div>
            <div class="flex items-center gap-2 shrink-0" @click.stop>
              <slot
                v-if="slots['collapsed-actions']"
                name="collapsed-actions"
                :open="open"
                :expand="expand"
                :collapse="collapse"
                :toggle="toggle"
              />
              <Button variant="secondary" @click="expand">{{ editLabel }}</Button>
            </div>
          </CardContent>
        </div>

        <div v-else key="expanded" class="w-full">
          <CardContent :class="expandedContentClass">
            <slot :open="open" :expand="expand" :collapse="collapse" :toggle="toggle" />
            <div v-if="slots.actions" :class="actionsClass">
              <slot name="actions" :open="open" :expand="expand" :collapse="collapse" :toggle="toggle" />
            </div>
          </CardContent>
        </div>
      </Transition>
    </div>
  </Card>
</template>
