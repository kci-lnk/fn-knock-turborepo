import { ref } from "vue";

export const useDelayedHostPopover = (closeDelayMs = 120) => {
  const openHost = ref<string | null>(null);
  let closeTimer: number | null = null;

  const clearCloseTimer = () => {
    if (closeTimer === null) return;
    window.clearTimeout(closeTimer);
    closeTimer = null;
  };

  const isOpen = (host: string): boolean => openHost.value === host;

  const open = (host: string) => {
    clearCloseTimer();
    openHost.value = host;
  };

  const scheduleClose = (host: string) => {
    if (openHost.value !== host) {
      return;
    }

    clearCloseTimer();
    closeTimer = window.setTimeout(() => {
      if (openHost.value === host) {
        openHost.value = null;
      }
      closeTimer = null;
    }, closeDelayMs);
  };

  const toggle = (host: string) => {
    clearCloseTimer();
    openHost.value = openHost.value === host ? null : host;
  };

  const handleOpenChange = (host: string, nextOpen: boolean) => {
    clearCloseTimer();

    if (nextOpen) {
      openHost.value = host;
      return;
    }

    if (openHost.value === host) {
      openHost.value = null;
    }
  };

  return {
    clearCloseTimer,
    handleOpenChange,
    isOpen,
    open,
    openHost,
    scheduleClose,
    toggle,
  };
};
