<script setup lang="ts">
import type { SelectTriggerProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { ChevronDown } from "lucide-vue-next"
import { injectSelectRootContext, SelectIcon, SelectTrigger, useForwardProps } from "reka-ui"
import { cn } from "@/lib/utils"

const props = withDefaults(
  defineProps<SelectTriggerProps & { class?: HTMLAttributes["class"], size?: "sm" | "default" }>(),
  { size: "default" },
)

const delegatedProps = reactiveOmit(props, "class", "size")
const forwardedProps = useForwardProps(delegatedProps)
const selectRootContext = injectSelectRootContext()

// Reka opens touch selects on pointerup. Some mobile WebViews still dispatch
// that pointerup after a native scroll gesture started on the trigger, which
// turns vertical page scrolling into an accidental select activation.
const TOUCH_SCROLL_THRESHOLD = 8
let activeTouchPointerId: number | null = null
let touchStartX = 0
let touchStartY = 0
let touchMoved = false
let shouldReplayTouchLikeMouse = false

function resetTouchGesture() {
  activeTouchPointerId = null
  touchMoved = false
  shouldReplayTouchLikeMouse = false
}

function isTouchLikePointer(event: PointerEvent) {
  if (event.pointerType === "touch")
    return true

  if (event.pointerType !== "mouse")
    return false

  const sourceCapabilities = (event as PointerEvent & {
    sourceCapabilities?: { firesTouchEvents?: boolean } | null
  }).sourceCapabilities
  if (sourceCapabilities?.firesTouchEvents === true)
    return true

  if ((event.width ?? 1) > 1 || (event.height ?? 1) > 1)
    return true

  // A few Android/Huawei WebViews report finger contacts as mouse pointers.
  // Only use maxTouchPoints as a fallback on coarse, non-hover devices so a
  // real mouse attached to a hybrid laptop keeps normal pointerdown behavior.
  if (window.navigator.maxTouchPoints <= 0)
    return false
  const finePointer = window.matchMedia?.("(hover: hover) and (pointer: fine)")
  return finePointer ? !finePointer.matches : true
}

function handleTouchPointerDownCapture(event: PointerEvent) {
  if (!isTouchLikePointer(event))
    return

  activeTouchPointerId = event.pointerId
  touchStartX = event.clientX
  touchStartY = event.clientY
  touchMoved = false
  shouldReplayTouchLikeMouse = event.pointerType === "mouse"

  if (shouldReplayTouchLikeMouse) {
    // Reka opens mouse pointers immediately on pointerdown. Defer a
    // touch-originated, misreported mouse contact until its movement is known.
    event.preventDefault()
    event.stopImmediatePropagation()
  }
}

function handleTouchPointerMoveCapture(event: PointerEvent) {
  if (event.pointerId !== activeTouchPointerId || touchMoved)
    return

  const deltaX = Math.abs(event.clientX - touchStartX)
  const deltaY = Math.abs(event.clientY - touchStartY)
  touchMoved = Math.max(deltaX, deltaY) >= TOUCH_SCROLL_THRESHOLD
}

function handleTouchPointerUpCapture(event: PointerEvent) {
  if (event.pointerId !== activeTouchPointerId)
    return

  const finalDeltaX = Math.abs(event.clientX - touchStartX)
  const finalDeltaY = Math.abs(event.clientY - touchStartY)
  const shouldSuppressOpen = touchMoved
    || Math.max(finalDeltaX, finalDeltaY) >= TOUCH_SCROLL_THRESHOLD
  const shouldReplayAsTouch = shouldReplayTouchLikeMouse
  resetTouchGesture()
  if (!shouldSuppressOpen && !shouldReplayAsTouch)
    return

  // This runs in the capture phase before Reka's pointerup handler. Stopping
  // the same event here keeps a completed scroll from opening the menu.
  event.preventDefault()
  event.stopImmediatePropagation()

  if (shouldReplayAsTouch && !shouldSuppressOpen) {
    // The original pointerdown was deliberately blocked before Reka could
    // mistake it for a mouse click. Open only now that this is known to be a
    // tap, and preserve the pointer position expected by SelectContent.
    selectRootContext.triggerPointerDownPosRef.value = {
      x: Math.round(event.pageX),
      y: Math.round(event.pageY),
    }
    selectRootContext.onOpenChange(true)
  }
}

function handleTouchPointerCancelCapture(event: PointerEvent) {
  if (event.pointerId === activeTouchPointerId)
    resetTouchGesture()
}
</script>

<template>
  <SelectTrigger
    data-slot="select-trigger"
    :data-size="size"
    v-bind="forwardedProps"
    @pointerdown.capture="handleTouchPointerDownCapture"
    @pointermove.capture="handleTouchPointerMoveCapture"
    @pointerup.capture="handleTouchPointerUpCapture"
    @pointercancel.capture="handleTouchPointerCancelCapture"
    :class="cn(
      'border-input data-[placeholder]:text-muted-foreground [&_svg:not([class*=\'text-\'])]:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:bg-input/30 dark:hover:bg-input/50 flex w-fit items-center justify-between gap-2 rounded-md border bg-transparent px-3 py-2 text-sm whitespace-nowrap shadow-xs transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-9 data-[size=sm]:h-8 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=\'size-\'])]:size-4',
      props.class,
    )"
  >
    <slot />
    <SelectIcon as-child>
      <ChevronDown class="size-4 opacity-50" />
    </SelectIcon>
  </SelectTrigger>
</template>
