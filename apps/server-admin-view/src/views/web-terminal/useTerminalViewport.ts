import { ref } from "vue";
import { useTerminalViewportLayout } from "./useTerminalViewportLayout";

const SIDEBAR_COLLAPSED_KEY = "fn-knock:terminal:sidebar-collapsed";

const readSidebarCollapsed = () => {
  try {
    return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "true";
  } catch {
    return false;
  }
};

export const useTerminalViewport = (options: {
  focusTerminal: () => void;
  scheduleFit: () => void;
  syncTerminalTextInputAnchor: () => void;
}) => {
  const layout = useTerminalViewportLayout(options);
  const targetDrawerOpen = ref(false);
  const sidebarCollapsed = ref(readSidebarCollapsed());

  const setTargetDrawerOpen = (open: boolean) => {
    targetDrawerOpen.value = open;
  };

  const closeTargetDrawer = () => {
    targetDrawerOpen.value = false;
  };

  const toggleSidebar = () => {
    sidebarCollapsed.value = !sidebarCollapsed.value;
    try {
      localStorage.setItem(
        SIDEBAR_COLLAPSED_KEY,
        String(sidebarCollapsed.value),
      );
    } catch {
      // View preference persistence is optional.
    }
    window.requestAnimationFrame(layout.syncViewportHeight);
  };

  return {
    ...layout,
    closeTargetDrawer,
    setTargetDrawerOpen,
    sidebarCollapsed,
    targetDrawerOpen,
    toggleSidebar,
  };
};
