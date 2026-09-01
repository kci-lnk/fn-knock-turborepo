import type { Ref } from "vue"
import { computed, getCurrentScope, onScopeDispose, ref, watch } from "vue"

const KEYBOARD_INSET_THRESHOLD = 80
const MOBILE_DIALOG_MAX_WIDTH = 640
const INPUT_SCROLL_RETRY_DELAYS = [120, 240, 240] as const
const NON_KEYBOARD_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "color",
  "date",
  "datetime-local",
  "file",
  "hidden",
  "image",
  "month",
  "radio",
  "range",
  "reset",
  "submit",
  "time",
  "week",
])

type MobileDialogInputFullscreenOptions = {
  isDialogOpen: Readonly<Ref<boolean>>
}

const isMobileDialogViewport = () => window.innerWidth < MOBILE_DIALOG_MAX_WIDTH

const isTextEntryElement = (
  target: Element | null,
  content: HTMLElement | null,
): target is HTMLElement => {
  if (!(target instanceof HTMLElement) || !content?.contains(target))
    return false

  if (target instanceof HTMLTextAreaElement) return !target.readOnly
  if (!(target instanceof HTMLInputElement)) return false

  return (
    !target.readOnly
    && target.inputMode !== "none"
    && !NON_KEYBOARD_INPUT_TYPES.has(target.type)
  )
}

const isScrollable = (element: HTMLElement) => {
  const overflowY = window.getComputedStyle(element).overflowY
  return /^(auto|overlay|scroll)$/u.test(overflowY)
}

const resolveScrollElement = (
  target: HTMLElement,
  content: HTMLElement,
) => {
  const explicitScrollElement = target.closest<HTMLElement>(
    "[data-dialog-input-scroll]",
  )
  if (explicitScrollElement && content.contains(explicitScrollElement))
    return explicitScrollElement

  let candidate = target.parentElement
  while (candidate && content.contains(candidate)) {
    if (isScrollable(candidate)) return candidate
    if (candidate === content) break
    candidate = candidate.parentElement
  }

  return content
}

export const useMobileDialogInputFullscreen = ({
  isDialogOpen,
}: MobileDialogInputFullscreenOptions) => {
  const contentElement = ref<HTMLElement | null>(null)
  const scrollElement = ref<HTMLElement | null>(null)
  const keyboardInset = ref(0)
  const inputFocused = ref(false)
  const keyboardSessionActive = ref(false)
  const viewportTop = ref("0px")
  const viewportHeight = ref("100dvh")
  let inputScrollTimer: number | null = null
  let inputSettleTimer: number | null = null
  let focusOutTimer: number | null = null
  let viewportListenersActive = false
  let observedViewport: VisualViewport | null = null

  const isInputFullscreen = computed(
    () =>
      keyboardSessionActive.value
      && (inputFocused.value || keyboardInset.value > 0),
  )
  const isSoftKeyboardVisible = computed(
    () => keyboardSessionActive.value && keyboardInset.value > 0,
  )
  const shouldScrollContent = computed(
    () =>
      isInputFullscreen.value
      && contentElement.value !== null
      && scrollElement.value === contentElement.value,
  )
  const contentStyle = computed(() => ({
    "--dialog-input-viewport-height": viewportHeight.value,
    "--dialog-input-viewport-top": viewportTop.value,
  }))

  const clearInputScrollTimers = () => {
    if (inputScrollTimer !== null) {
      window.clearTimeout(inputScrollTimer)
      inputScrollTimer = null
    }
    if (inputSettleTimer !== null) {
      window.clearTimeout(inputSettleTimer)
      inputSettleTimer = null
    }
  }

  const clearFocusOutTimer = () => {
    if (focusOutTimer === null) return
    window.clearTimeout(focusOutTimer)
    focusOutTimer = null
  }

  const resolveKeyboardInset = () => {
    const viewport = window.visualViewport
    if (!viewport) return 0
    const inset = window.innerHeight - viewport.height - viewport.offsetTop
    return inset > KEYBOARD_INSET_THRESHOLD ? Math.ceil(inset) : 0
  }

  const updateViewport = () => {
    const viewport = window.visualViewport
    viewportTop.value = viewport
      ? `${Math.max(0, viewport.offsetTop)}px`
      : "0px"
    viewportHeight.value = viewport
      ? `${Math.max(0, viewport.height)}px`
      : "100dvh"
    keyboardInset.value = isDialogOpen.value ? resolveKeyboardInset() : 0

    if (!inputFocused.value && keyboardInset.value === 0)
      keyboardSessionActive.value = false
  }

  const scrollInputIntoView = (
    target: HTMLElement,
    behavior: ScrollBehavior = "smooth",
  ) => {
    updateViewport()

    const container = scrollElement.value
    if (!container) {
      target.scrollIntoView({ block: "center", inline: "nearest", behavior })
      return
    }

    const targetRect = target.getBoundingClientRect()
    const containerRect = container.getBoundingClientRect()
    const viewport = window.visualViewport
    const visibleViewportTop = viewport?.offsetTop ?? 0
    const visibleViewportBottom = viewport
      ? viewport.offsetTop + viewport.height
      : window.innerHeight
    const visibleTop = Math.max(containerRect.top, visibleViewportTop + 12)
    const visibleBottom = Math.min(
      containerRect.bottom,
      visibleViewportBottom - 16,
    )
    const visibleHeight = visibleBottom - visibleTop

    if (visibleHeight <= 0) {
      target.scrollIntoView({ block: "center", inline: "nearest", behavior })
      return
    }

    const desiredCenter = visibleTop + visibleHeight / 2
    const targetCenter = targetRect.top + targetRect.height / 2
    const maxScrollTop = Math.max(
      0,
      container.scrollHeight - container.clientHeight,
    )
    const nextScrollTop = Math.min(
      maxScrollTop,
      Math.max(0, container.scrollTop + targetCenter - desiredCenter),
    )

    if (typeof container.scrollTo === "function") {
      container.scrollTo({ top: nextScrollTop, behavior })
    }
    else {
      container.scrollTop = nextScrollTop
    }

    if (inputSettleTimer !== null) window.clearTimeout(inputSettleTimer)
    inputSettleTimer = window.setTimeout(() => {
      inputSettleTimer = null
      if (!isDialogOpen.value || !target.isConnected) return
      target.scrollIntoView({ block: "center", inline: "nearest", behavior })
    }, 0)
  }

  const scheduleInputScrollIntoView = (target: HTMLElement) => {
    clearInputScrollTimers()

    let attempt = 0
    const run = () => {
      scrollInputIntoView(target, attempt === 0 ? "auto" : "smooth")
      const delay = INPUT_SCROLL_RETRY_DELAYS[attempt]
      attempt += 1
      if (delay === undefined) {
        inputScrollTimer = null
        return
      }
      inputScrollTimer = window.setTimeout(run, delay)
    }

    run()
  }

  const handleViewportChange = () => {
    if (!isMobileDialogViewport()) {
      reset()
      return
    }

    const activeElement = document.activeElement
    const hasFocusedInput =
      isDialogOpen.value
      && isTextEntryElement(activeElement, contentElement.value)
    inputFocused.value = hasFocusedInput
    updateViewport()
    if (!hasFocusedInput) {
      if (!isInputFullscreen.value) stopViewportListeners()
      return
    }

    keyboardSessionActive.value = true
    scheduleInputScrollIntoView(activeElement)
  }

  const stopViewportListeners = () => {
    if (!viewportListenersActive) return
    observedViewport?.removeEventListener("resize", handleViewportChange)
    observedViewport?.removeEventListener("scroll", handleViewportChange)
    observedViewport = null
    viewportListenersActive = false
  }

  const startViewportListeners = () => {
    const viewport = window.visualViewport
    if (viewportListenersActive || !viewport) return
    observedViewport = viewport
    viewport.addEventListener("resize", handleViewportChange)
    viewport.addEventListener("scroll", handleViewportChange)
    viewportListenersActive = true
  }

  const reset = () => {
    clearInputScrollTimers()
    clearFocusOutTimer()
    stopViewportListeners()
    contentElement.value = null
    scrollElement.value = null
    keyboardInset.value = 0
    inputFocused.value = false
    keyboardSessionActive.value = false
    viewportTop.value = "0px"
    viewportHeight.value = "100dvh"
  }

  const handleFocusIn = (event: FocusEvent) => {
    if (!isMobileDialogViewport()) return

    const content = event.currentTarget
    const target = event.target as Element | null
    if (
      !(content instanceof HTMLElement)
      || !isTextEntryElement(target, content)
    )
      return

    contentElement.value = content
    scrollElement.value = resolveScrollElement(target, content)
    clearFocusOutTimer()
    inputFocused.value = true
    keyboardSessionActive.value = true
    updateViewport()
    startViewportListeners()
    scheduleInputScrollIntoView(target)
  }

  const handleFocusOut = (event: FocusEvent) => {
    const target = event.target as Element | null
    if (!isTextEntryElement(target, contentElement.value)) return

    clearInputScrollTimers()
    clearFocusOutTimer()
    focusOutTimer = window.setTimeout(() => {
      focusOutTimer = null
      inputFocused.value = isTextEntryElement(
        document.activeElement,
        contentElement.value,
      )
      updateViewport()
      if (!isInputFullscreen.value) stopViewportListeners()
    }, 0)
  }

  watch(isDialogOpen, (open) => {
    if (!open) reset()
  })

  if (getCurrentScope()) onScopeDispose(reset)

  return {
    contentStyle,
    handleFocusIn,
    handleFocusOut,
    handleViewportChange,
    isInputFullscreen,
    isSoftKeyboardVisible,
    reset,
    shouldScrollContent,
  }
}
