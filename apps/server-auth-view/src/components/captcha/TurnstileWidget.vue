<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';

const props = defineProps<{
  siteKey: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  verified: [token: string];
  expired: [];
  reset: [];
  error: [message: string];
}>();

type TurnstileRenderOptions = {
  sitekey: string;
  size: 'normal';
  callback: (token: string) => void;
  'expired-callback': () => void;
  'error-callback': () => void;
  'timeout-callback': () => void;
};

type TurnstileApi = {
  render: (container: HTMLElement, options: TurnstileRenderOptions) => string;
  reset: (widgetId?: string) => void;
  remove: (widgetId?: string) => void;
};

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

const TURNSTILE_SCRIPT_SRC = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';

let turnstileScriptPromise: Promise<void> | null = null;

const ensureTurnstileScript = async () => {
  if (typeof window === 'undefined') return;
  if (window.turnstile) return;
  if (turnstileScriptPromise) {
    await turnstileScriptPromise;
    return;
  }

  turnstileScriptPromise = new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(`script[src="${TURNSTILE_SCRIPT_SRC}"]`);
    if (existing) {
      existing.addEventListener('load', () => resolve(), { once: true });
      existing.addEventListener('error', () => reject(new Error('Turnstile 脚本加载失败')), { once: true });
      return;
    }

    const script = document.createElement('script');
    script.src = TURNSTILE_SCRIPT_SRC;
    script.async = true;
    script.defer = true;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error('Turnstile 脚本加载失败'));
    document.head.appendChild(script);
  });

  await turnstileScriptPromise;
};

const containerRef = ref<HTMLDivElement | null>(null);
let widgetId: string | null = null;
let renderVersion = 0;

const clearWidget = () => {
  if (typeof window !== 'undefined' && window.turnstile && widgetId) {
    window.turnstile.remove(widgetId);
  }
  widgetId = null;
  if (containerRef.value) {
    containerRef.value.innerHTML = '';
  }
};

const resetWidget = () => {
  if (typeof window !== 'undefined' && window.turnstile && widgetId) {
    window.turnstile.reset(widgetId);
  }
  emit('reset');
};

const renderWidget = async () => {
  const siteKey = props.siteKey.trim();
  if (!siteKey || !containerRef.value) {
    clearWidget();
    emit('reset');
    return;
  }

  const currentVersion = ++renderVersion;

  try {
    await ensureTurnstileScript();
  } catch (error: any) {
    emit('error', error?.message || 'Turnstile 脚本加载失败');
    return;
  }

  if (currentVersion !== renderVersion || !containerRef.value || !window.turnstile) {
    return;
  }

  clearWidget();
  emit('reset');

  widgetId = window.turnstile.render(containerRef.value, {
    sitekey: siteKey,
    size: 'normal',
    callback: (token: string) => {
      emit('verified', token);
    },
    'expired-callback': () => {
      emit('expired');
      resetWidget();
    },
    'error-callback': () => {
      emit('error', 'Turnstile 渲染失败，请稍后重试');
    },
    'timeout-callback': () => {
      emit('error', 'Turnstile 验证超时，请重试');
    },
  });
};

watch(() => props.siteKey, () => {
  void renderWidget();
});

watch(() => props.disabled, (disabled) => {
  if (disabled) {
    resetWidget();
  }
});

onMounted(() => {
  void renderWidget();
});

onBeforeUnmount(() => {
  renderVersion += 1;
  clearWidget();
});

defineExpose({
  reset: resetWidget,
  reload: () => {
    void renderWidget();
  },
});
</script>

<template>
  <div ref="containerRef" class="mx-auto min-h-[68px] w-full max-w-[320px]" />
</template>
