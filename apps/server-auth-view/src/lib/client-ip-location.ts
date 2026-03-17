import { onBeforeUnmount, ref } from 'vue';
import type { AuthClientInfo, AuthClientLocationStatus } from '@frontend-core/auth/types';
import { AuthAPI } from './api';

const POLL_INTERVAL_MS = 2000;

export const useClientIpLocation = () => {
  const clientIp = ref('');
  const ipLocation = ref('');
  const ipLocationStatus = ref<AuthClientLocationStatus>('idle');

  let pollTimer: ReturnType<typeof window.setTimeout> | null = null;

  const clearPollTimer = () => {
    if (pollTimer) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const scheduleNextPoll = () => {
    clearPollTimer();
    pollTimer = window.setTimeout(() => {
      void refreshLocation();
    }, POLL_INTERVAL_MS);
  };

  const refreshLocation = async () => {
    if (!clientIp.value) return;

    try {
      const location = await AuthAPI.getClientLocation();
      clientIp.value = location.ip || clientIp.value;
      ipLocation.value = location.location || '';
      ipLocationStatus.value = location.status;

      if (location.status === 'queued' || location.status === 'processing') {
        scheduleNextPoll();
        return;
      }
    } catch {
      ipLocationStatus.value = 'failed';
    }

    clearPollTimer();
  };

  const startLocationPolling = (client: AuthClientInfo) => {
    clientIp.value = client.ip;
    ipLocation.value = '';
    ipLocationStatus.value = 'idle';
    clearPollTimer();
    void refreshLocation();
  };

  onBeforeUnmount(() => {
    clearPollTimer();
  });

  return {
    clientIp,
    ipLocation,
    ipLocationStatus,
    refreshLocation,
    startLocationPolling,
    stopLocationPolling: clearPollTimer,
  };
};
