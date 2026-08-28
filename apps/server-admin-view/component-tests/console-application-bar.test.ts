import { DOMWrapper, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { nextTick, ref } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../src/types";
import { useConfigStore } from "../src/store/config";
import ConsoleApplicationBar from "../src/views/layout/ConsoleApplicationBar.vue";

const accessEntry = vi.hoisted(() => ({
  load: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/composables/useAccessEntryPort", () => ({
  useAccessEntryPort: () => ({
    accessEntryPort: ref("7999"),
    loadAccessEntryPort: accessEntry.load,
  }),
}));

const i18n = createI18n({
  legacy: false,
  locale: "en",
  messages: {
    en: {
      admin: {
        consoleApplicationList: {
          label: "Apps",
          ariaLabel: "Console application list",
          empty: "No applications available",
          openApplication: "Open {name} in a new tab",
        },
      },
    },
  },
});

const pathConfig = (): AppConfig =>
  ({
    runtime_profile: { deployment_target: "fpk" },
    dashboard_display: { show_console_app_list: true },
    run_type: 1,
    reverse_proxy_submode: "path",
    host_mappings: [],
    host_mapping_groups: [],
    host_mapping_grouped_view: false,
    proxy_mappings: [
      {
        path: "/photos",
        target: "http://photos:3000",
        rewrite_html: false,
        use_auth: true,
        use_root_mode: false,
        strip_path: false,
      },
    ],
  }) as AppConfig;

const hostConfig = (faviconOverride: string, showAppIcon = true): AppConfig =>
  ({
    ...pathConfig(),
    run_type: 3,
    gateway_portal: {
      enabled: true,
      display_style: "title",
      show_app_icon: showAppIcon,
      show_wol: true,
      icon_drag_mode: "corners",
      version: "v1",
    },
    host_mappings: [
      {
        host: "photos.example.test",
        service_role: "app",
        disabled: false,
        group_id: null,
        title: "Photos",
        title_override: "",
        favicon: "",
        favicon_override: faviconOverride,
      },
    ],
    proxy_mappings: [],
  }) as AppConfig;

describe("ConsoleApplicationBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
    document.body.replaceChildren();
  });

  const openDialog = async (wrapper: ReturnType<typeof mount>) => {
    await wrapper.get("button").trigger("click");
    await nextTick();
    const content = document.body.querySelector<HTMLElement>(
      '[data-slot="dialog-content"]',
    );
    expect(content).not.toBeNull();
    return new DOMWrapper(content!);
  };

  it("opens an application dialog with a responsive new-tab application grid", async () => {
    const store = useConfigStore();
    store.config = pathConfig();
    const wrapper = mount(ConsoleApplicationBar, {
      global: { plugins: [i18n] },
    });

    const nav = wrapper.get("nav");
    expect(nav.attributes("aria-label")).toBe("Console application list");
    expect(nav.classes()).toContain("max-w-full");
    expect(nav.classes()).toContain("overflow-hidden");
    expect(nav.classes()).toContain("shadow-none");
    expect(wrapper.get("ul").classes()).toContain("overflow-x-auto");
    expect(wrapper.get("ul").classes()).toContain("cursor-grab");
    expect(wrapper.get("ul").classes()).toContain("snap-x");
    const barLink = wrapper.get("ul a");
    expect(barLink.attributes("href")).toMatch(/:7999\/photos\/$/u);

    const trigger = wrapper.get("button");
    expect(trigger.attributes("aria-label")).toBe("Console application list");
    expect(trigger.attributes("aria-expanded")).toBe("false");
    const dialog = await openDialog(wrapper);
    expect(trigger.attributes("aria-expanded")).toBe("true");
    expect(dialog.text()).toContain("Apps");

    const grid = dialog.get('[data-testid="console-application-grid"]');
    expect(grid.classes()).toContain(
      "grid-cols-[repeat(auto-fit,minmax(min(100%,8rem),1fr))]",
    );
    const link = grid.get("a");
    expect(link.attributes("target")).toBe("_blank");
    expect(link.attributes("rel")).toBe("noopener noreferrer");
    expect(link.attributes("aria-label")).toBe("Open /photos in a new tab");
    expect(link.attributes("href")).toMatch(/:7999\/photos\/$/u);
    expect(link.attributes("title")).toBe("/photos");
    expect(link.attributes("title")).not.toContain("photos:3000");
    expect(accessEntry.load).toHaveBeenCalledTimes(1);

    const focusSpy = vi.spyOn(trigger.element, "focus");
    await dialog.get('[data-slot="dialog-close"]').trigger("click");
    await nextTick();
    expect(
      document.body.querySelector('[data-slot="dialog-content"]'),
    ).toBeNull();
    expect(focusSpy).toHaveBeenCalledOnce();
    expect(focusSpy).toHaveBeenCalledWith({ preventScroll: true });
  });

  it("shows the empty state inside the dialog when no application is configured", async () => {
    const store = useConfigStore();
    store.config = { ...pathConfig(), proxy_mappings: [] };
    const wrapper = mount(ConsoleApplicationBar, {
      global: { plugins: [i18n] },
    });

    expect(wrapper.text()).toContain("No applications available");
    const dialog = await openDialog(wrapper);
    expect(dialog.text()).toContain("No applications available");
    expect(dialog.find("a").exists()).toBe(false);
  });

  it("does not render or load the gateway entry outside FPK", () => {
    const store = useConfigStore();
    store.config = {
      ...pathConfig(),
      runtime_profile: {
        deployment_target: "docker",
      },
    } as AppConfig;
    const wrapper = mount(ConsoleApplicationBar, {
      global: { plugins: [i18n] },
    });

    expect(wrapper.find("button").exists()).toBe(false);
    expect(accessEntry.load).not.toHaveBeenCalled();
  });

  it("retries a Host icon after its source changes", async () => {
    const firstIcon = "data:image/png;base64,Zmlyc3Q=";
    const nextIcon = "data:image/png;base64,c2Vjb25k";
    const store = useConfigStore();
    store.config = hostConfig(firstIcon);
    const wrapper = mount(ConsoleApplicationBar, {
      global: { plugins: [i18n] },
    });
    const dialog = await openDialog(wrapper);

    await dialog.get("img").trigger("error");
    expect(dialog.find("img").exists()).toBe(false);

    store.config = hostConfig(nextIcon);
    await nextTick();
    expect(dialog.get("img").attributes("src")).toBe(nextIcon);
  });

  it("hides the Host icon completely when portal icons are disabled", async () => {
    const store = useConfigStore();
    store.config = hostConfig("", false);
    const wrapper = mount(ConsoleApplicationBar, {
      global: { plugins: [i18n] },
    });
    const dialog = await openDialog(wrapper);

    const link = dialog.get("a");
    expect(link.find("img").exists()).toBe(false);
    expect(link.find("svg").exists()).toBe(false);
  });
});
