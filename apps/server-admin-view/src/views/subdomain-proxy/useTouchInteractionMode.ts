import { ref } from "vue";

const TOUCH_INTERACTION_QUERY = "(hover: none), (pointer: coarse)";

export const useTouchInteractionMode = () => {
  const isTouchInteraction = ref(false);
  let interactionMediaQuery: MediaQueryList | null = null;

  const updateInteractionMode = () => {
    if (typeof window === "undefined") {
      return;
    }

    isTouchInteraction.value = window.matchMedia(
      TOUCH_INTERACTION_QUERY,
    ).matches;
  };

  const startTouchInteractionTracking = () => {
    if (typeof window === "undefined") {
      return;
    }

    interactionMediaQuery = window.matchMedia(TOUCH_INTERACTION_QUERY);
    updateInteractionMode();

    if (typeof interactionMediaQuery.addEventListener === "function") {
      interactionMediaQuery.addEventListener("change", updateInteractionMode);
    } else {
      interactionMediaQuery.addListener(updateInteractionMode);
    }
  };

  const stopTouchInteractionTracking = () => {
    if (!interactionMediaQuery) {
      return;
    }

    if (typeof interactionMediaQuery.removeEventListener === "function") {
      interactionMediaQuery.removeEventListener(
        "change",
        updateInteractionMode,
      );
    } else {
      interactionMediaQuery.removeListener(updateInteractionMode);
    }
    interactionMediaQuery = null;
  };

  return {
    isTouchInteraction,
    startTouchInteractionTracking,
    stopTouchInteractionTracking,
    updateInteractionMode,
  };
};
