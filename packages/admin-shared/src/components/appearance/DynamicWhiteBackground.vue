<template>
  <div v-if="props.active" class="dynamic-white-background" aria-hidden="true">
    <svg class="dynamic-white-background__filters" focusable="false">
      <defs>
        <filter
          id="dynamic-white-liquid-glass-filter"
          x="-12%"
          y="-12%"
          width="124%"
          height="124%"
          color-interpolation-filters="sRGB"
        >
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.026 0.018"
            numOctaves="2"
            seed="7"
            result="dynamicWhiteNoise"
          />
          <feGaussianBlur
            in="dynamicWhiteNoise"
            stdDeviation="0.55"
            result="dynamicWhiteSoftNoise"
          />
          <feComponentTransfer
            in="dynamicWhiteSoftNoise"
            result="dynamicWhiteDisplacementMap"
          >
            <feFuncR
              type="gamma"
              amplitude="1.8"
              exponent="0.72"
              offset="-0.18"
            />
            <feFuncG
              type="gamma"
              amplitude="1.8"
              exponent="0.72"
              offset="-0.18"
            />
          </feComponentTransfer>
          <feDisplacementMap
            in="SourceGraphic"
            in2="dynamicWhiteDisplacementMap"
            scale="46"
            xChannelSelector="R"
            yChannelSelector="G"
          />
        </filter>
      </defs>
    </svg>
    <div
      :id="DYNAMIC_WHITE_BACKGROUND_DOM_ID"
      class="dynamic-white-background__canvas"
    ></div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, watch } from "vue";
import abstractShapeBgUrl from "../../assets/lib/AbstractShapeBg.min.js?url";

type AbstractShapeBgOptions = {
  dom: string;
  colors: string[];
  loop: boolean;
};

type AbstractShapeBgInstance = {
  _update?: () => void;
  canvasManager?: {
    destroy?: () => void;
  };
  destroy?: () => void;
  loop?: boolean;
  remove?: () => void;
};

declare global {
  interface Window {
    Color4Bg?: {
      AbstractShapeBg: new (
        options: AbstractShapeBgOptions,
      ) => AbstractShapeBgInstance;
    };
  }
}

const props = defineProps<{
  active: boolean;
}>();

const DYNAMIC_WHITE_BACKGROUND_DOM_ID = "box";
const DYNAMIC_WHITE_BACKGROUND_COLORS = [
  "#f5f5f5",
  "#e3e3e3",
  "#e8e8e8",
  "#ebebeb",
  "#f0f0f0",
  "#ffffff",
];

let activationToken = 0;
let backgroundInstance: AbstractShapeBgInstance | null = null;
let scriptLoadPromise: Promise<void> | null = null;

const hasWebGLSupport = () => {
  try {
    const canvas = document.createElement("canvas");
    return Boolean(
      canvas.getContext("webgl") ||
      canvas.getContext("experimental-webgl") ||
      canvas.getContext("webgl2"),
    );
  } catch {
    return false;
  }
};

const ensureAbstractShapeScript = () => {
  if (window.Color4Bg?.AbstractShapeBg) return Promise.resolve();
  if (scriptLoadPromise) return scriptLoadPromise;

  scriptLoadPromise = new Promise<void>((resolve, reject) => {
    const script = document.createElement("script");
    script.src = abstractShapeBgUrl;
    script.async = true;
    script.dataset.fnKnockAbstractShapeBg = "true";
    script.onload = () => resolve();
    script.onerror = () => {
      scriptLoadPromise = null;
      reject(new Error("Failed to load dynamic white background script"));
    };
    document.head.appendChild(script);
  });

  return scriptLoadPromise;
};

const clearBackgroundContainer = () => {
  document.getElementById(DYNAMIC_WHITE_BACKGROUND_DOM_ID)?.replaceChildren();
};

const disposeBackgroundInstance = () => {
  try {
    if (backgroundInstance) {
      backgroundInstance.loop = false;
      backgroundInstance._update = () => {};
    }
    backgroundInstance?.canvasManager?.destroy?.();
    backgroundInstance?.destroy?.();
    backgroundInstance?.remove?.();
  } catch (error) {
    console.warn("Failed to dispose dynamic white background:", error);
  } finally {
    backgroundInstance = null;
    clearBackgroundContainer();
  }
};

const initializeBackground = async (isStillActive: () => boolean) => {
  const currentToken = ++activationToken;

  if (!hasWebGLSupport()) {
    console.warn("WebGL not supported, skipping dynamic white background");
    return;
  }

  try {
    await ensureAbstractShapeScript();
    await nextTick();

    const container = document.getElementById(DYNAMIC_WHITE_BACKGROUND_DOM_ID);
    if (!isStillActive() || currentToken !== activationToken || !container) {
      return;
    }

    disposeBackgroundInstance();
    backgroundInstance = new window.Color4Bg!.AbstractShapeBg({
      dom: DYNAMIC_WHITE_BACKGROUND_DOM_ID,
      colors: [...DYNAMIC_WHITE_BACKGROUND_COLORS],
      loop: true,
    });
  } catch (error) {
    console.warn("Failed to initialize dynamic white background:", error);
  }
};

watch(
  () => props.active,
  (isActive) => {
    activationToken += 1;

    if (!isActive) {
      disposeBackgroundInstance();
      return;
    }

    void initializeBackground(() => props.active);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  activationToken += 1;
  disposeBackgroundInstance();
});
</script>

<style scoped>
.dynamic-white-background {
  pointer-events: none;
  position: fixed;
  inset: 0;
  z-index: -10;
  overflow: hidden;
  background: #ffffff;
}

.dynamic-white-background__canvas {
  position: absolute;
  inset: 0;
}

.dynamic-white-background__filters {
  position: absolute;
  width: 0;
  height: 0;
}
</style>
