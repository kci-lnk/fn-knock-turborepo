import { onMounted, onUnmounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { driver } from 'driver.js';
import { toast } from '@admin-shared/utils/toast';
import { ConfigAPI } from '../lib/api';

type GuidePhase = 'idle' | 'run-mode' | 'auth' | 'ssl' | 'done';

const waitForElement = async (selector: string, timeoutMs = 6000): Promise<HTMLElement | null> => {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const el = document.querySelector<HTMLElement>(selector);
    if (el) return el;
    await new Promise((resolve) => window.setTimeout(resolve, 120));
  }
  return null;
};

export const useOnboardingGuide = () => {
  const router = useRouter();
  const route = useRoute();
  const phase = ref<GuidePhase>('idle');
  let activeDriver: ReturnType<typeof driver> | null = null;
  let showing = false;
  let onboardingMarked = false;

  const driverOptions = {
    showProgress: true,
    allowClose: true,
    disableActiveInteraction: false,
    overlayColor: 'rgba(8, 16, 32, 0.62)',
    stagePadding: 24,
    stageRadius: 18,
    popoverClass: 'driverjs-theme',
  };

  const closeDriver = () => {
    activeDriver?.destroy();
    activeDriver = null;
    showing = false;
  };

  const gotoPhase = async (next: Exclude<GuidePhase, 'idle'>) => {
    closeDriver();
    phase.value = next;
    if (next === 'run-mode' && route.path !== '/mode') {
      await router.push('/mode');
    }
    if (next === 'auth' && route.path !== '/auth') {
      await router.push('/auth');
    }
    if (next === 'ssl' && route.path !== '/ssl') {
      await router.push('/ssl');
    }
  };

  const markCompletedOnFirstPopup = async () => {
    if (onboardingMarked) return;
    onboardingMarked = true;
    try {
      await ConfigAPI.completeOnboarding();
    } catch (error) {
      console.error('Failed to mark onboarding completed on first popup:', error);
    }
  };

  const showRunModeGuide = async () => {
    if (showing) return;
    const target = await waitForElement('[data-guide-run-mode]');
    if (!target || phase.value !== 'run-mode' || route.path !== '/mode') return;
    showing = true;
    void markCompletedOnFirstPopup();
    activeDriver = driver(driverOptions);
    activeDriver.highlight({
      element: target,
      popover: {
        title: '步骤 1/3：先选择运行模式',
        description: '你有公网 IP 就选直连模式；没有公网 IP 就选反代模式。选择后点击“保存修改”，再继续下一步。',
        side: 'bottom',
        align: 'start',
        showButtons: ['next', 'close'],
        nextBtnText: '下一步',
        onNextClick: () => {
          void gotoPhase('auth');
        },
      },
    });
  };

  const showAuthGuide = async () => {
    if (showing) return;
    const target = await waitForElement('[data-guide-auth-settings]');
    if (!target || phase.value !== 'auth' || route.path !== '/auth') return;
    showing = true;
    activeDriver = driver(driverOptions);
    activeDriver.highlight({
      element: target,
      popover: {
        title: '步骤 2/3：绑定 TOTP 令牌',
        description: '在这里点击“绑定新令牌”完成双重验证配置，建议至少绑定一个常用设备。',
        side: 'bottom',
        align: 'start',
        showButtons: ['next', 'close'],
        nextBtnText: '下一步',
        onNextClick: () => {
          void gotoPhase('ssl');
        },
      },
    });
  };

  const showSslGuide = async () => {
    if (showing) return;
    const target = await waitForElement('[data-guide-ssl-settings]');
    if (!target || phase.value !== 'ssl' || route.path !== '/ssl') return;
    showing = true;
    activeDriver = driver(driverOptions);
    activeDriver.highlight({
      element: target,
      popover: {
        title: '步骤 3/3：配置 SSL 证书',
        description: '你可以在证书配置页上传证书，或使用自签/ACME 自动签发，提升访问安全性。',
        side: 'bottom',
        align: 'start',
        showButtons: ['next', 'close'],
        nextBtnText: '完成',
        onNextClick: () => {
          phase.value = 'done';
          closeDriver();
          toast.success('新手向导已完成');
        },
      },
    });
  };

  const maybeShowCurrentPhaseGuide = () => {
    if (phase.value === 'run-mode' && route.path === '/mode') {
      void showRunModeGuide();
      return;
    }
    if (phase.value === 'auth' && route.path === '/auth') {
      void showAuthGuide();
      return;
    }
    if (phase.value === 'ssl' && route.path === '/ssl') {
      void showSslGuide();
    }
  };

  onMounted(async () => {
    try {
      const status = await ConfigAPI.getOnboardingStatus();
      if (status.completed) {
        phase.value = 'done';
        return;
      }
      await gotoPhase('run-mode');
      maybeShowCurrentPhaseGuide();
    } catch (error) {
      console.error('Failed to load onboarding status:', error);
    }
  });

  watch(
    () => route.path,
    () => {
      if (phase.value === 'done') return;
      showing = false;
      maybeShowCurrentPhaseGuide();
    },
  );

  onUnmounted(() => {
    closeDriver();
  });
};
