import type { ComputedRef, WritableComputedRef } from "vue"
import { computed, nextTick } from "vue"
import { useColorMode } from "@vueuse/core"

export const THEME_MODE_STORAGE_KEY = "fn-knock:theme-mode"

export type ThemeMode = "light" | "dark"
export type ResolvedThemeMode = "light" | "dark"

type ThemeModeState = {
  mode: WritableComputedRef<ThemeMode>
  resolvedMode: ComputedRef<ResolvedThemeMode>
  setThemeMode: (value: ThemeMode) => void
  toggleThemeMode: (event?: MouseEvent) => Promise<void>
}

let sharedThemeModeState: ThemeModeState | null = null
let activeThemeTransition: Promise<void> | null = null

type ViewTransitionLike = {
  finished: Promise<void>
}

type DocumentWithViewTransition = Document & {
  startViewTransition?: (
    updateCallback: () => void | Promise<void>,
  ) => ViewTransitionLike
}

const isThemeMode = (value: unknown): value is ThemeMode =>
  value === "light" || value === "dark"

export const normalizeThemeMode = (value: unknown): ThemeMode | null =>
  isThemeMode(value) ? value : null

const THEME_TRANSITION_STYLE_ID = "fn-knock-theme-transition-style"
const THEME_TRANSITION_DURATION = "1s"
const THEME_TRANSITION_MASK =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 40 40'%3E%3Ccircle cx='20' cy='20' r='20' fill='white'/%3E%3C/svg%3E\")"

const readStoredThemeMode = (): ThemeMode | null => {
  if (typeof window === "undefined") return null

  try {
    return normalizeThemeMode(window.localStorage.getItem(THEME_MODE_STORAGE_KEY))
  } catch {
    return null
  }
}

const applyResolvedThemeMode = (value: ResolvedThemeMode) => {
  if (typeof document === "undefined") return

  const root = document.documentElement
  root.classList.toggle("dark", value === "dark")
  root.style.colorScheme = value
}

export const applyStoredThemeMode = () => {
  applyResolvedThemeMode(readStoredThemeMode() ?? "light")
}

const prefersReducedMotion = () => {
  if (typeof window === "undefined") return true

  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
}

const ensureThemeTransitionStyles = () => {
  if (typeof document === "undefined") return
  if (document.getElementById(THEME_TRANSITION_STYLE_ID)) return

  const style = document.createElement("style")
  style.id = THEME_TRANSITION_STYLE_ID
  style.textContent = `
:root {
  --fn-knock-theme-transition-duration: ${THEME_TRANSITION_DURATION};
  --fn-knock-theme-transition-mask: ${THEME_TRANSITION_MASK};
  --fn-knock-theme-expo-out: linear(
    0 0%, 0.1684 2.66%, 0.3165 5.49%, 0.446 8.52%,
    0.5581 11.78%, 0.6535 15.29%, 0.7341 19.11%,
    0.8011 23.3%, 0.8557 27.93%, 0.8962 32.68%,
    0.9283 38.01%, 0.9529 44.08%, 0.9711 51.14%,
    0.9833 59.06%, 0.9915 68.74%, 1 100%
  );
}

:root[data-theme-transitioning] *,
:root[data-theme-transitioning] *::before,
:root[data-theme-transitioning] *::after {
  transition-property: none !important;
}

::view-transition-group(root) {
  animation-timing-function: var(--fn-knock-theme-expo-out);
}

::view-transition-old(root),
.dark::view-transition-old(root) {
  animation: none;
  animation-fill-mode: both;
  z-index: -1;
}

::view-transition-new(root),
.dark::view-transition-new(root) {
  animation: fn-knock-theme-reveal var(--fn-knock-theme-transition-duration);
  animation-fill-mode: both;
  animation-timing-function: var(--fn-knock-theme-expo-out);
  -webkit-mask: var(--fn-knock-theme-transition-mask) center / 0 no-repeat;
  mask: var(--fn-knock-theme-transition-mask) center / 0 no-repeat;
}

@keyframes fn-knock-theme-reveal {
  to {
    -webkit-mask-size: 200vmax;
    mask-size: 200vmax;
  }
}
`
  document.head.appendChild(style)
}

const setTransitionAttributes = (targetMode: ThemeMode) => {
  const root = document.documentElement
  root.dataset.themeTransition = targetMode === "dark" ? "to-dark" : "to-light"
  root.dataset.themeTransitioning = ""
}

const clearTransitionAttributes = () => {
  const root = document.documentElement
  delete root.dataset.themeTransition
  delete root.dataset.themeTransitioning
}

const runThemeTransition = async (
  targetMode: ThemeMode,
  applyTheme: () => void | Promise<void>,
) => {
  const startViewTransition =
    typeof document === "undefined"
      ? undefined
      : (document as DocumentWithViewTransition).startViewTransition?.bind(
          document,
        )

  if (
    typeof document === "undefined" ||
    typeof window === "undefined" ||
    prefersReducedMotion() ||
    !startViewTransition
  ) {
    await applyTheme()
    return
  }

  ensureThemeTransitionStyles()
  setTransitionAttributes(targetMode)

  try {
    const transition = startViewTransition(applyTheme)
    await transition.finished.catch(() => undefined)
  } finally {
    clearTransitionAttributes()
  }
}

export const useThemeMode = (): ThemeModeState => {
  if (sharedThemeModeState) return sharedThemeModeState

  const colorMode = useColorMode<ThemeMode>({
    selector: "html",
    attribute: "class",
    initialValue: "light",
    storageKey: THEME_MODE_STORAGE_KEY,
    modes: {
      light: "",
      dark: "dark",
    },
    onChanged(mode, defaultHandler) {
      const resolvedMode = mode === "dark" ? "dark" : "light"
      defaultHandler(resolvedMode)
      applyResolvedThemeMode(resolvedMode)
    },
  })

  if (!normalizeThemeMode(colorMode.store.value)) {
    colorMode.store.value = "light"
  }

  const mode = computed<ThemeMode>({
    get: () => normalizeThemeMode(colorMode.store.value) ?? "light",
    set: (value) => {
      colorMode.store.value = value
    },
  })

  const resolvedMode = computed<ResolvedThemeMode>(() =>
    colorMode.state.value === "dark" ? "dark" : "light",
  )

  const setThemeMode = (value: ThemeMode) => {
    mode.value = value
  }

  const toggleThemeMode = async (event?: MouseEvent) => {
    void event

    if (activeThemeTransition) {
      await activeThemeTransition
      return
    }

    const nextMode = mode.value === "dark" ? "light" : "dark"
    activeThemeTransition = runThemeTransition(
      nextMode,
      async () => {
        setThemeMode(nextMode)
        await nextTick()
      },
    ).finally(() => {
      activeThemeTransition = null
    })

    await activeThemeTransition
  }

  sharedThemeModeState = {
    mode,
    resolvedMode,
    setThemeMode,
    toggleThemeMode,
  }

  return sharedThemeModeState
}
