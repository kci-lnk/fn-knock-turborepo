import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LayoutDashboard, BellRing } from "lucide-vue-next";
import type { AppConfig, DashboardDisplayConfig } from "../src/types";

const api = vi.hoisted(() => ({ updateDashboardDisplayConfig: vi.fn() }));
const toast = vi.hoisted(() => ({ error: vi.fn(), success: vi.fn() }));
vi.mock("@/lib/api/config", () => ({ ConfigAPI: api }));
vi.mock("@admin-shared/utils/toast", () => ({ toast }));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));

import { useConfigStore } from "../src/store/config";
import LayoutDesktopSidebar from "../src/views/layout/LayoutDesktopSidebar.vue";
import { DEFAULT_SIDEBAR_MENU_ORDER } from "../src/views/layout/sidebarNavigation";

const displayConfig = (collapsed = false): DashboardDisplayConfig => ({
  sidebar_collapsed: collapsed,
  show_entry_status_module: false,
  show_console_app_list: true,
  sidebar_menu_order: [...DEFAULT_SIDEBAR_MENU_ORDER],
  date_time_display_mode: "full",
});

const wrappers: ReturnType<typeof mount>[] = [];
const mountSidebar = (
  options: { collapsed?: boolean; loading?: boolean; legacy?: boolean } = {},
) => {
  const pinia = createPinia();
  setActivePinia(pinia);
  const store = useConfigStore();
  const display: Partial<DashboardDisplayConfig> = displayConfig(
    options.collapsed,
  );
  if (options.legacy) delete display.sidebar_collapsed;
  store.config = { dashboard_display: display } as AppConfig;
  store.isLoading = options.loading ?? false;
  const navigateTo = vi.fn().mockResolvedValue(undefined);
  const onOpenLocale = vi.fn();
  const wrapper = mount(LayoutDesktopSidebar, {
    attachTo: document.body,
    props: {
      navItems: [
        {
          id: "dashboard",
          name: "Dashboard",
          path: "/",
          icon: LayoutDashboard,
        },
        {
          id: "events",
          name: "Events",
          path: "/events",
          icon: BellRing,
          alert: "Critical events present",
        },
      ],
      currentVersionLabel: "v2.4.15",
      isNavActive: (path: string) => path === "/events",
      isSidebarMenuOrderMode: true,
      shouldShowPanelLogout: true,
      isLogoutSubmitting: false,
      navigateTo,
      onPanelLogout: vi.fn().mockResolvedValue(undefined),
      onOpenLocale,
    },
    global: {
      plugins: [pinia],
      stubs: {
        ThemeModeToggle: {
          template: '<button aria-label="Theme">Theme</button>',
        },
        ConfirmDangerPopover: { template: '<slot name="trigger" />' },
      },
    },
  });
  wrappers.push(wrapper);
  return { wrapper, store, navigateTo, onOpenLocale };
};

const toggleButton = (wrapper: ReturnType<typeof mount>) =>
  wrapper.get('button[aria-controls="desktop-sidebar-menu"]');

beforeEach(() => vi.resetAllMocks());
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  document.body.innerHTML = "";
});

describe("desktop sidebar collapse", () => {
  it("defaults legacy configurations to expanded and waits for configuration readiness", async () => {
    const { wrapper, store } = mountSidebar({ legacy: true, loading: true });
    expect(wrapper.attributes("data-collapsed")).toBe("false");
    expect(toggleButton(wrapper).attributes("disabled")).toBeDefined();
    await toggleButton(wrapper).trigger("click");
    expect(api.updateDashboardDisplayConfig).not.toHaveBeenCalled();
    store.isLoading = false;
    await nextTick();
    expect(toggleButton(wrapper).attributes("disabled")).toBeUndefined();
    store.isError = true;
    await nextTick();
    expect(toggleButton(wrapper).attributes("disabled")).toBeDefined();
  });

  it("applies the server preference when loading finishes", async () => {
    const { wrapper, store } = mountSidebar({ loading: true });
    store.config!.dashboard_display = displayConfig(true);
    store.isLoading = false;
    await nextTick();
    expect(wrapper.attributes("data-collapsed")).toBe("true");
    expect(wrapper.classes()).toContain("sm:w-16");
    expect(toggleButton(wrapper).attributes("aria-expanded")).toBe("false");
    expect(wrapper.get('button[aria-current="page"]').text()).not.toContain(
      "Events",
    );
  });

  it("changes immediately, writes only the preference and prevents duplicate submissions", async () => {
    let resolve!: (value: DashboardDisplayConfig) => void;
    api.updateDashboardDisplayConfig.mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const { wrapper, store } = mountSidebar();
    await toggleButton(wrapper).trigger("click");
    expect(wrapper.attributes("data-collapsed")).toBe("true");
    expect(toggleButton(wrapper).attributes("disabled")).toBeDefined();
    await toggleButton(wrapper).trigger("click");
    expect(api.updateDashboardDisplayConfig).toHaveBeenCalledTimes(1);
    expect(api.updateDashboardDisplayConfig).toHaveBeenCalledWith({
      sidebar_collapsed: true,
    });
    resolve(displayConfig(true));
    await flushPromises();
    expect(store.config!.dashboard_display).toEqual(displayConfig(true));
    expect(toggleButton(wrapper).attributes("disabled")).toBeUndefined();
    expect(toast.success).not.toHaveBeenCalled();

    api.updateDashboardDisplayConfig.mockResolvedValueOnce(
      displayConfig(false),
    );
    await toggleButton(wrapper).trigger("click");
    await flushPromises();
    expect(api.updateDashboardDisplayConfig).toHaveBeenLastCalledWith({
      sidebar_collapsed: false,
    });
    expect(wrapper.attributes("data-collapsed")).toBe("false");
  });

  it("restores the previous state and reports a failed save", async () => {
    api.updateDashboardDisplayConfig.mockRejectedValueOnce(
      new Error("Network unavailable"),
    );
    const { wrapper, store } = mountSidebar({ collapsed: true });
    await toggleButton(wrapper).trigger("click");
    await flushPromises();
    expect(wrapper.attributes("data-collapsed")).toBe("true");
    expect(store.config!.dashboard_display!.sidebar_collapsed).toBe(true);
    expect(toggleButton(wrapper).attributes("disabled")).toBeUndefined();
    expect(toast.error).toHaveBeenCalledWith("admin.nav.sidebarSaveFailed", {
      description: "Network unavailable",
    });
  });

  it("uses the latest server preference after a save fails during a config refresh", async () => {
    let reject!: (error: Error) => void;
    api.updateDashboardDisplayConfig.mockReturnValueOnce(
      new Promise((_done, fail) => {
        reject = fail;
      }),
    );
    const { wrapper, store } = mountSidebar();
    await toggleButton(wrapper).trigger("click");
    store.config!.dashboard_display = displayConfig(true);
    await nextTick();
    reject(new Error("Network unavailable"));
    await flushPromises();
    expect(wrapper.attributes("data-collapsed")).toBe("true");
    expect(store.config!.dashboard_display!.sidebar_collapsed).toBe(true);
  });

  it("removes tooltip descriptions when the sidebar expands while a menu stays focused", async () => {
    const { wrapper, store } = mountSidebar({ collapsed: true });
    const dashboard = wrapper.get('button[aria-label="Dashboard"]');
    (dashboard.element as HTMLButtonElement).focus();
    await flushPromises();
    expect(dashboard.attributes("aria-describedby")).toBeTruthy();
    store.config!.dashboard_display = displayConfig(false);
    await flushPromises();
    expect(dashboard.attributes("aria-describedby")).toBeUndefined();
    expect(document.activeElement).toBe(dashboard.element);
  });

  it("restores the saved server state in a fresh client", async () => {
    api.updateDashboardDisplayConfig.mockResolvedValueOnce(displayConfig(true));
    const first = mountSidebar();
    await toggleButton(first.wrapper).trigger("click");
    await flushPromises();
    const second = mountSidebar({
      collapsed: first.store.config!.dashboard_display!.sidebar_collapsed,
    });
    expect(second.wrapper.attributes("data-collapsed")).toBe("true");
    expect(api.updateDashboardDisplayConfig).toHaveBeenCalledTimes(1);
  });

  it("keeps navigation, current-page state, alerts and ordering hints in icon mode", async () => {
    const { wrapper, navigateTo } = mountSidebar({ collapsed: true });
    const events = wrapper.get('button[aria-current="page"]');
    expect(events.attributes("aria-current")).toBe("page");
    expect(events.find(".absolute.right-1.top-1").exists()).toBe(true);
    expect(
      wrapper
        .get(
          ".sidebar-menu-editing .layout-scroll-area__viewport > button[aria-current='page']",
        )
        .exists(),
    ).toBe(true);
    await events.trigger("click");
    expect(navigateTo).toHaveBeenCalledWith("/events");
  });

  it("shows menu and alert text on focus in a portal outside the scroll area", async () => {
    const { wrapper } = mountSidebar({ collapsed: true });
    const events = wrapper.get('button[aria-current="page"]');
    (events.element as HTMLButtonElement).focus();
    await flushPromises();
    const description = document.getElementById(
      events.attributes("aria-describedby")!,
    );
    expect(description?.textContent).toContain("Events");
    expect(description?.textContent).toContain("Critical events present");
    expect(description?.closest(".layout-scroll-area__viewport")).toBeNull();
    expect(wrapper.element.contains(description)).toBe(false);
  });

  it("shows a menu tooltip on mouse hover", async () => {
    const { wrapper } = mountSidebar({ collapsed: true });
    const dashboard = wrapper.get('button[aria-label="Dashboard"]');
    await dashboard.trigger("pointermove", { pointerType: "mouse" });
    await vi.waitFor(() => {
      expect(document.querySelector('[role="tooltip"]')?.textContent).toContain(
        "Dashboard",
      );
    });
  });

  it("keeps expanded labels and avoids redundant menu tooltips", async () => {
    const { wrapper } = mountSidebar();
    const dashboard = wrapper.get('button[aria-label="Dashboard"]');
    expect(dashboard.text()).toBe("Dashboard");
    (dashboard.element as HTMLButtonElement).focus();
    await flushPromises();
    expect(dashboard.attributes("aria-describedby")).toBeUndefined();
    expect(
      wrapper.get(".layout-scroll-area__viewport--rail-gutter").exists(),
    ).toBe(true);
  });

  it("keeps footer controls accessible and sends the original locale trigger event", async () => {
    const { wrapper, onOpenLocale } = mountSidebar({ collapsed: true });
    const locale = wrapper.get('button[aria-label="locale.label"]');
    expect(locale.element.parentElement?.classList.contains("flex-col")).toBe(
      true,
    );
    await locale.trigger("click");
    expect(onOpenLocale.mock.calls[0]![0]).toBeInstanceOf(MouseEvent);
    expect(
      wrapper.find('button[aria-label="admin.dockerAdmin.logout"]').exists(),
    ).toBe(true);
    const website = wrapper.get('a[aria-label="admin.nav.officialWebsite"]');
    expect(website.text()).toBe("");
    (website.element as HTMLAnchorElement).focus();
    await flushPromises();
    expect(document.querySelector('[role="tooltip"]')?.textContent).toContain(
      "v2.4.15",
    );
  });
});
