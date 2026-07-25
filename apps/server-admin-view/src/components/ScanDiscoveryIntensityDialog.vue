<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent
      :show-close-button="false"
      class="w-auto max-w-[calc(100vw-2rem)] gap-0 border-0 bg-transparent p-0 shadow-none sm:max-w-none"
    >
      <DialogTitle class="sr-only">
        {{ t("admin.scanIntensity.title") }}
      </DialogTitle>

      <section
        class="scan-pressure-card"
        :class="{
          'is-energized': displayedIndex === 3,
          'is-fallback': graphicsFallback,
        }"
        :aria-busy="loading || saving"
      >
        <div class="scan-pressure-heading">
          <div class="scan-pressure-reading">
            <span class="scan-pressure-label">
              {{ t("admin.scanIntensity.level") }}
            </span>
            <span
              class="scan-pressure-value"
              :class="{ 'is-energized': displayedIndex === 3 }"
            >
              {{ displayedOption.label }}
            </span>
          </div>

          <TooltipProvider>
            <Tooltip
              :open="concurrencyPopupOpen"
              @update:open="handleConcurrencyPopupOpenChange"
            >
              <TooltipTrigger as-child>
                <button
                  type="button"
                  class="scan-pressure-help"
                  :disabled="loading || disabled || safeConcurrency === null"
                  :title="currentConcurrencyText"
                  :aria-label="currentConcurrencyText"
                  @click.stop="toggleConcurrencyPopup"
                >
                  <CircleHelp aria-hidden="true" />
                </button>
              </TooltipTrigger>
              <TooltipContent align="end" class="scan-pressure-popup">
                <p class="scan-pressure-popup-primary">
                  {{ currentConcurrencyText }}
                </p>
                <p class="scan-pressure-popup-secondary">
                  {{
                    t("admin.scanIntensity.concurrency", {
                      count: displayedOption.concurrency,
                    })
                  }}
                </p>
                <p class="scan-pressure-popup-secondary">
                  {{
                    t("admin.scanIntensity.safeConcurrency", {
                      count: safeConcurrency,
                    })
                  }}
                </p>
                <button
                  v-if="!automatic"
                  type="button"
                  class="scan-pressure-popup-action"
                  :disabled="saving"
                  @click.stop="restoreAutomaticMode"
                >
                  {{ t("admin.scanIntensity.autoTitle") }}
                </button>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <div class="scan-pressure-track">
          <div class="scan-pressure-visual" aria-hidden="true">
            <div class="scan-pressure-track-base"></div>
            <div class="scan-pressure-markers">
              <i v-for="marker in 5" :key="marker"></i>
            </div>
            <canvas
              :ref="setMatrixCanvas"
              class="scan-pressure-canvas"
            ></canvas>
          </div>
          <span class="scan-pressure-terminal-shield" aria-hidden="true"></span>
          <input
            :value="sliderPosition"
            class="scan-pressure-input"
            type="range"
            min="0"
            max="100"
            step="1"
            :disabled="loading || saving || disabled"
            :aria-label="t('admin.scanIntensity.sliderLabel')"
            :aria-valuetext="displayedOption.label"
            @input="handleSliderInput"
            @change="flushManualSave"
          />
          <span
            class="scan-pressure-handle"
            :style="sliderHandleStyle"
            aria-hidden="true"
          ></span>
        </div>
      </section>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CircleHelp } from "lucide-vue-next";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { ScanDiscoverySettings } from "@/lib/api";
import { useScanDiscoveryIntensitySettings } from "@/composables/useScanDiscoveryIntensitySettings";
import { useScanIntensityMatrix } from "@/composables/useScanIntensityMatrix";

const props = withDefaults(
  defineProps<{
    open: boolean;
    disabled?: boolean;
  }>(),
  { disabled: false },
);

const emit = defineEmits<{
  "update:open": [value: boolean];
  saved: [settings: ScanDiscoverySettings];
}>();

const { t } = useI18n();
const concurrencyPopupOpen = ref(false);
const {
  loading,
  saving,
  automatic,
  sliderPosition,
  safeConcurrency,
  displayedIndex,
  displayedOption,
  currentConcurrencyText,
  loadSettings,
  handleSliderInput,
  flushManualSave,
  clearPendingSave,
  restoreAutomaticMode: restoreAutomaticSettings,
} = useScanDiscoveryIntensitySettings({
  disabled: () => props.disabled,
  onSaved: (settings) => emit("saved", settings),
});
const {
  setCanvas: setMatrixCanvas,
  isFallback: graphicsFallback,
  shutdown: shutdownPortMatrix,
} = useScanIntensityMatrix({
  active: () => props.open,
  tier: displayedIndex,
});

const sliderHandleStyle = computed<Record<string, string>>(() => {
  if (sliderPosition.value <= 0) {
    return {
      left: "0px",
      right: "auto",
      "--scan-handle-offset": "0px",
    };
  }
  if (sliderPosition.value >= 100) {
    return {
      left: "auto",
      right: "0px",
      "--scan-handle-offset": "0px",
    };
  }
  return {
    left: `${sliderPosition.value}%`,
    right: "auto",
    "--scan-handle-offset": `${-(sliderPosition.value / 100) * 29}px`,
  };
});

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    await loadSettings();
  },
  { immediate: true },
);

function handleOpenChange(value: boolean) {
  if (!value) closeDialog();
}

function handleConcurrencyPopupOpenChange(value: boolean) {
  concurrencyPopupOpen.value = value;
}

function toggleConcurrencyPopup() {
  if (loading.value || props.disabled) return;
  concurrencyPopupOpen.value = !concurrencyPopupOpen.value;
}

function restoreAutomaticMode() {
  concurrencyPopupOpen.value = false;
  restoreAutomaticSettings();
}

function closeDialog() {
  clearPendingSave();
  concurrencyPopupOpen.value = false;
  shutdownPortMatrix();
  emit("update:open", false);
}
</script>

<style scoped>
.scan-pressure-card {
  width: min(376px, calc(100vw - 2rem));
  user-select: none;
  border: 1px solid rgb(255 255 255 / 12%);
  border-radius: 20px;
  background: #000;
  padding: 18px 20px 16px;
  color: #f4f4f5;
  box-shadow:
    0 12px 28px rgb(0 0 0 / 20%),
    0 4px 12px rgb(0 0 0 / 10%);
}

.scan-pressure-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}

.scan-pressure-reading {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  font-size: 16px;
  line-height: 1.3;
  perspective: 280px;
  perspective-origin: center 120%;
}

.scan-pressure-label {
  color: #b0b0c7;
  font-weight: 700;
}

.scan-pressure-value {
  display: inline-block;
  color: #a1a1aa;
  font-weight: 500;
  transition:
    color 0.3s,
    text-shadow 0.3s;
  transform-origin: center bottom;
}

.scan-pressure-value.is-energized {
  color: #c084fc;
  font-weight: 600;
  text-shadow: 0 0 12px rgb(168 85 247 / 60%);
  animation: scan-pressure-rise 0.42s cubic-bezier(0.33, 1, 0.68, 1);
}

@keyframes scan-pressure-rise {
  from {
    opacity: 0;
    filter: blur(4px);
    transform: translateY(18px) rotateX(-80deg);
  }
  to {
    opacity: 1;
    filter: blur(0);
    transform: translateY(0) rotateX(0);
  }
}

.scan-pressure-help {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 0;
  background: transparent;
  padding: 0;
  color: #a1a1aa;
  cursor: pointer;
  transition:
    color 0.2s,
    opacity 0.2s;
}

.scan-pressure-help:hover,
.scan-pressure-help:focus-visible {
  color: #d4d4d8;
}

.scan-pressure-help:focus-visible {
  border-radius: 999px;
  outline: 2px solid #a855f7;
  outline-offset: 3px;
}

.scan-pressure-help:disabled {
  cursor: default;
  opacity: 0.55;
}

.scan-pressure-help svg {
  width: 18px;
  height: 18px;
  shape-rendering: geometricprecision;
}

.scan-pressure-popup {
  min-width: 150px;
  padding: 8px 10px;
  text-align: left;
}

.scan-pressure-popup-primary {
  font-weight: 650;
}

.scan-pressure-popup-secondary {
  margin-top: 2px;
  opacity: 0.72;
}

.scan-pressure-popup-action {
  width: 100%;
  margin-top: 7px;
  border-top: 1px solid rgb(255 255 255 / 16%);
  padding-top: 6px;
  color: #d8b4fe;
  text-align: left;
  cursor: pointer;
}

.scan-pressure-popup-action:hover,
.scan-pressure-popup-action:focus-visible {
  color: #f3e8ff;
}

.scan-pressure-popup-action:focus-visible {
  outline: none;
  text-decoration: underline;
}

.scan-pressure-popup-action:disabled {
  cursor: default;
  opacity: 0.5;
}

.scan-pressure-track {
  position: relative;
  isolation: isolate;
  height: 30px;
  border-radius: 10px;
  overflow: visible;
}

.scan-pressure-visual {
  position: absolute;
  z-index: 0;
  inset: 0;
  overflow: hidden;
  border: 1px solid #1a1a1e;
  border-radius: inherit;
  background: #0c0c0c;
}

.scan-pressure-track-base,
.scan-pressure-markers,
.scan-pressure-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.scan-pressure-track-base {
  z-index: 0;
  background: linear-gradient(135deg, #111113, #0a0a0b);
}

.scan-pressure-markers {
  z-index: 1;
  pointer-events: none;
}

.scan-pressure-markers i {
  position: absolute;
  top: 50%;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #494950;
  transform: translateY(-50%);
  transition: opacity 0.6s;
}

.scan-pressure-markers i:nth-child(1) {
  left: 10%;
}
.scan-pressure-markers i:nth-child(2) {
  left: 30%;
}
.scan-pressure-markers i:nth-child(3) {
  left: 50%;
}
.scan-pressure-markers i:nth-child(4) {
  left: 70%;
}
.scan-pressure-markers i:nth-child(5) {
  left: 90%;
}

.scan-pressure-canvas {
  z-index: 2;
  display: block;
  pointer-events: none;
  opacity: 0;
  mix-blend-mode: screen;
  transition: opacity 0.3s;
}

.scan-pressure-card.is-energized .scan-pressure-canvas {
  z-index: 4;
  opacity: 1;
}

.scan-pressure-card.is-energized .scan-pressure-markers i {
  opacity: 0;
}

.scan-pressure-card.is-fallback.is-energized .scan-pressure-track-base {
  background:
    radial-gradient(circle, rgb(245 222 255 / 95%) 0 2px, transparent 2.5px) 0
      0 / 7px 7px,
    linear-gradient(90deg, #160b27, #4c1d79 45%, #f4e9ff 88%, #120b1d);
}

.scan-pressure-terminal-shield {
  position: absolute;
  z-index: 5;
  top: -1px;
  right: -1px;
  width: 17px;
  height: 32px;
  background: #000;
  opacity: 0;
  pointer-events: none;
}

.scan-pressure-card.is-energized .scan-pressure-terminal-shield {
  opacity: 1;
}

.scan-pressure-input {
  position: absolute;
  z-index: 7;
  inset: 0;
  box-sizing: border-box;
  width: 100%;
  height: 30px;
  margin: 0;
  padding: 0;
  border: 0;
  outline: none;
  appearance: none;
  background: transparent;
  cursor: pointer;
}

.scan-pressure-input:disabled {
  cursor: default;
}

.scan-pressure-input::-webkit-slider-runnable-track {
  height: 30px;
  background: transparent;
}

.scan-pressure-input::-webkit-slider-thumb {
  width: 29px;
  height: 29px;
  appearance: none;
  border: 0;
  background: transparent;
  box-shadow: none;
  cursor: grab;
}

.scan-pressure-input::-webkit-slider-thumb:active {
  cursor: grabbing;
}

.scan-pressure-handle {
  --scan-handle-offset: 0px;
  position: absolute;
  z-index: 6;
  top: 50%;
  box-sizing: border-box;
  width: 29px;
  height: 29px;
  border: 0.5px solid rgb(0 0 0 / 8%);
  border-radius: 10px;
  background: linear-gradient(170deg, #fff 0%, #f0f0f2 40%, #e4e4e6 100%);
  box-shadow:
    0 0.5px 1px rgb(0 0 0 / 18%),
    0 2px 6px rgb(0 0 0 / 25%),
    0 6px 16px rgb(0 0 0 / 12%),
    inset 0 0.5px 0 rgb(255 255 255 / 85%),
    inset 0 -0.5px 0 rgb(0 0 0 / 6%);
  pointer-events: none;
  transform: translate(var(--scan-handle-offset), -50%);
  transform-origin: center;
  backface-visibility: hidden;
  transition:
    box-shadow 0.4s ease,
    transform 0.15s ease;
}

.scan-pressure-input:active + .scan-pressure-handle {
  transform: translate(var(--scan-handle-offset), -50%) scale(0.95);
}

.scan-pressure-card.is-energized .scan-pressure-handle {
  box-shadow:
    0 0.5px 1px rgb(0 0 0 / 18%),
    0 2px 6px rgb(0 0 0 / 25%),
    0 6px 16px rgb(0 0 0 / 12%),
    0 0 28px rgb(168 85 247 / 50%),
    0 0 50px rgb(168 85 247 / 25%),
    inset 0 0.5px 0 rgb(255 255 255 / 85%),
    inset 0 -0.5px 0 rgb(0 0 0 / 6%);
}

.scan-pressure-input::-moz-range-track {
  height: 30px;
  border: 0;
  background: transparent;
}

.scan-pressure-input::-moz-range-thumb {
  width: 29px;
  height: 29px;
  border: 0;
  background: transparent;
  box-shadow: none;
  cursor: grab;
}

.scan-pressure-input::-moz-range-thumb:active {
  cursor: grabbing;
}

.scan-pressure-input:focus-visible {
  outline: 2px solid #c084fc;
  outline-offset: -3px;
}

@media (max-width: 420px) {
  .scan-pressure-card {
    padding-inline: 18px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .scan-pressure-value.is-energized {
    animation: none;
  }
  .scan-pressure-canvas,
  .scan-pressure-markers i,
  .scan-pressure-handle {
    transition: none;
  }
}
</style>
