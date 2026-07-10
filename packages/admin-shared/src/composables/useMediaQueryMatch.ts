import { onBeforeUnmount, onMounted, ref } from "vue";

/**
 * Reactively tracks a media query while preserving compatibility with embedded
 * browsers that only expose the legacy MediaQueryList listener API.
 */
export function useMediaQueryMatch(query: string) {
  const matches = ref(false);
  let mediaQuery: MediaQueryList | null = null;

  const updateMatches = () => {
    matches.value = Boolean(mediaQuery?.matches);
  };

  onMounted(() => {
    if (typeof window === "undefined") return;

    mediaQuery = window.matchMedia(query);
    updateMatches();

    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", updateMatches);
      return;
    }

    mediaQuery.addListener(updateMatches);
  });

  onBeforeUnmount(() => {
    if (!mediaQuery) return;

    if (typeof mediaQuery.removeEventListener === "function") {
      mediaQuery.removeEventListener("change", updateMatches);
    } else {
      mediaQuery.removeListener(updateMatches);
    }

    mediaQuery = null;
  });

  return matches;
}
