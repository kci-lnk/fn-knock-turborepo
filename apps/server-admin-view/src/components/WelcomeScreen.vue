<template>
  <Transition name="fade-overlay">
    <div v-if="visible" class="welcome-wrapper">
      <div class="welcome-container">
        <div class="hello-box">
          <svg
            class="hello-svg"
            viewBox="-64 -56 1358.94 526.57"
            preserveAspectRatio="xMidYMid meet"
          >
            <defs>
              <linearGradient id="helloGradient" x1="0%" y1="0%" x2="100%" y2="0%">
                <stop offset="0%" style="stop-color: #76a4ff" />
                <stop offset="50%" style="stop-color: #ff9fed" />
                <stop offset="100%" style="stop-color: #ffbf8a" />
              </linearGradient>
            </defs>
            <path
              class="hello-path"
              d="M-293.58-104.62S-103.61-205.49-60-366.25c9.13-32.45,9-58.31,0-74-10.72-18.82-49.69-33.21-75.55,31.94-27.82,70.11-52.22,377.24-44.11,322.48s34-176.24,99.89-183.19c37.66-4,49.55,23.58,52.83,47.92a117.06,117.06,0,0,1-3,45.32c-7.17,27.28-20.47,97.67,33.51,96.86,66.93-1,131.91-53.89,159.55-84.49,31.1-36.17,31.1-70.64,19.27-90.25-16.74-29.92-69.47-33-92.79,16.73C62.78-179.86,98.7-93.8,159-81.63S302.7-99.55,393.3-269.92c29.86-58.16,52.85-114.71,46.14-150.08-7.44-39.21-59.74-54.5-92.87-8.7-47,65-61.78,266.62-34.74,308.53S416.62-58,481.52-130.31s133.2-188.56,146.54-256.23c14-71.15-56.94-94.64-88.4-47.32C500.53-375,467.58-229.49,503.3-127a73.73,73.73,0,0,0,23.43,33.67c25.49,20.23,55.1,16,77.46,6.32a111.25,111.25,0,0,0,30.44-19.87c37.73-34.23,29-36.71,64.58-127.53C724-284.3,785-298.63,821-259.13a71,71,0,0,1,13.69,22.56c17.68,46,6.81,80-6.81,107.89-12,24.62-34.56,42.72-61.45,47.91-23.06,4.45-48.37-.35-66.48-24.27a78.88,78.88,0,0,1-12.66-25.8c-14.75-51,4.14-88.76,11-101.41,6.18-11.39,37.26-69.61,103.42-42.24,55.71,23.05,100.66-23.31,100.66-23.31"
              transform="translate(311.08 476.02)"
            />
          </svg>
        </div>

        <div class="action-box" :class="{ 'show-btn': showButton }">
          <button class="ios-btn" :disabled="pending" @click="handleStart">
            {{
              pending
                ? t("admin.welcomeScreen.entering")
                : t("admin.welcomeScreen.start")
            }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = withDefaults(
  defineProps<{
    visible?: boolean;
    pending?: boolean;
  }>(),
  {
    visible: true,
    pending: false,
  },
);

const emit = defineEmits<{
  start: [];
}>();

const { t } = useI18n();
const showButton = ref(false);
let revealButtonTimer: number | null = null;

const clearRevealButtonTimer = () => {
  if (revealButtonTimer !== null) {
    window.clearTimeout(revealButtonTimer);
    revealButtonTimer = null;
  }
};

watch(
  () => props.visible,
  (visible) => {
    clearRevealButtonTimer();
    if (!visible) {
      showButton.value = false;
      return;
    }

    showButton.value = false;
    revealButtonTimer = window.setTimeout(() => {
      showButton.value = true;
    }, 4500);
  },
  { immediate: true },
);

onUnmounted(() => {
  clearRevealButtonTimer();
});

const handleStart = () => {
  if (props.pending) return;
  emit("start");
};
</script>

<style scoped>
.welcome-wrapper {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(5, 7, 14, 0.96);
  z-index: 9999;
  display: flex;
  justify-content: center;
  align-items: center;
  overflow: hidden;
  isolation: isolate;
  backdrop-filter: blur(0px) saturate(100%);
  -webkit-backdrop-filter: blur(0px) saturate(100%);
  animation: overlay-soften 4.8s cubic-bezier(0.22, 1, 0.36, 1) forwards;
}

.welcome-wrapper::before,
.welcome-wrapper::after {
  content: "";
  position: absolute;
  pointer-events: none;
}

.welcome-wrapper::before {
  inset: -18%;
  background:
    radial-gradient(circle at 18% 18%, rgba(118, 164, 255, 0.44), transparent 34%),
    radial-gradient(circle at 78% 24%, rgba(255, 159, 237, 0.34), transparent 30%),
    radial-gradient(circle at 52% 78%, rgba(255, 191, 138, 0.28), transparent 34%);
  opacity: 0.12;
  filter: blur(10px);
  transform: scale(1.08);
  animation: ambient-haze 4.8s cubic-bezier(0.22, 1, 0.36, 1) forwards;
}

.welcome-wrapper::after {
  inset: 0;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.06), transparent 30%),
    linear-gradient(180deg, rgba(3, 5, 12, 0.12), rgba(3, 5, 12, 0.28));
  opacity: 0.58;
}

@keyframes overlay-soften {
  0% {
    background: rgba(0, 0, 0, 1);
    backdrop-filter: blur(0px) saturate(100%);
    -webkit-backdrop-filter: blur(0px) saturate(100%);
  }
  35% {
    background: rgba(8, 10, 18, 0.88);
    backdrop-filter: blur(6px) saturate(112%);
    -webkit-backdrop-filter: blur(6px) saturate(112%);
  }
  100% {
    background: rgba(8, 10, 18, 0.62);
    backdrop-filter: blur(24px) saturate(138%);
    -webkit-backdrop-filter: blur(24px) saturate(138%);
  }
}

@keyframes ambient-haze {
  0% {
    opacity: 0;
    filter: blur(0px);
    transform: scale(1.12);
  }
  30% {
    opacity: 0.9;
  }
  100% {
    opacity: 0.72;
    filter: blur(42px);
    transform: scale(1);
  }
}

.welcome-container {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  width: min(92vw, 680px);
  max-width: 600px;
  padding: clamp(20px, 5vw, 32px);
  box-sizing: border-box;
}

.hello-box {
  width: 100%;
  padding-inline: clamp(8px, 3vw, 20px);
  box-sizing: border-box;
}

.hello-svg {
  display: block;
  width: 100%;
  height: auto;
  overflow: visible;
}

.hello-path {
  fill: none;
  stroke: url(#helloGradient);
  stroke-width: 40px;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-dasharray: 5800;
  stroke-dashoffset: 5800;
  animation: draw-hello 5s cubic-bezier(0.45, 0, 0.55, 1) forwards;
}

@keyframes draw-hello {
  0% { stroke-dashoffset: 5800; }
  20% { stroke-dashoffset: 5800; }
  100% { stroke-dashoffset: 0; }
}

.action-box {
  margin-top: 50px;
  opacity: 0;
  transform: translateY(20px);
  transition: all 0.8s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.action-box.show-btn {
  opacity: 1;
  transform: translateY(0);
}

.ios-btn {
  padding: 12px 48px;
  font-size: 18px;
  font-weight: 500;
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 25px;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(15px);
  -webkit-backdrop-filter: blur(15px);
  box-shadow: 0 4px 15px rgba(0, 0, 0, 0.3);
  transition:
    transform 0.2s ease,
    background 0.3s ease,
    opacity 0.2s ease;
  outline: none;
}

.ios-btn:disabled {
  cursor: wait;
  opacity: 0.72;
}

.ios-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.ios-btn:active {
  transform: scale(0.95);
  background: rgba(255, 255, 255, 0.15);
}

.fade-overlay-leave-active {
  pointer-events: none;
  transition: opacity 0.8s ease;
}
.fade-overlay-leave-to {
  opacity: 0;
}

@media (max-width: 768px) {
  .welcome-container {
    width: min(100vw, 680px);
    padding: 18px 12px 24px;
  }
  .hello-path {
    stroke-width: 34px;
  }
  .ios-btn {
    width: auto;
    min-width: clamp(160px, 44vw, 220px);
    max-width: 100%;
    padding: 12px 28px;
    font-size: 16px;
    border-radius: 22px;
  }
  .action-box {
    width: min(100%, 620px);
    display: flex;
    justify-content: center;
    margin-top: 34px;
  }
}
</style>
