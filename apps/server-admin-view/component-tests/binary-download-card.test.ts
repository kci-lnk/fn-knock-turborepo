import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { createI18n } from "vue-i18n";
import { describe, expect, it, vi } from "vitest";
import BinaryDownloadCard from "@admin-shared/components/system/BinaryDownloadCard.vue";

const passthrough = defineComponent({
  setup(_, { attrs, slots }) {
    return () => h("div", attrs, slots.default?.());
  },
});

const resourceCard = defineComponent({
  setup(_, { attrs, slots }) {
    return () =>
      h("section", attrs, [slots.default?.(), slots.footer?.()].flat());
  },
});

const popover = defineComponent({
  setup(_, { slots }) {
    return () => h("div", slots.default?.({ close: vi.fn() }));
  },
});

const button = defineComponent({
  inheritAttrs: false,
  setup(_, { attrs, slots }) {
    return () => h("button", attrs, slots.default?.());
  },
});

const mountCard = (
  installationStatus: "missing" | "outdated" | "current",
  installedVersion: string | null = null,
) =>
  mount(BinaryDownloadCard, {
    props: {
      title: "Cloudflared",
      description: "description",
      isInitializing: false,
      supported: true,
      platform: "linux-amd64",
      downloaded: installationStatus === "current",
      installationStatus,
      installedVersion,
      targetVersion: "2026.11.2",
      status: "idle",
      percent: 0,
      readyLabel: "Ready",
      pendingLabel: "Missing",
      downloadButtonText: "Download",
      outdatedLabel: "Outdated",
      outdatedTitle: "Old Cloudflared",
      outdatedDescription: "The tunnel reconnects during the update.",
      updateButtonText: "Update now",
      updateConfirmTitle: "Update Cloudflared?",
      updateConfirmDescription: "The tunnel reconnects briefly.",
      redownloadConfirmTitle: "Download again?",
      redownloadConfirmDescription: "Overwrite it.",
      deleteConfirmTitle: "Delete it?",
      deleteConfirmDescription: "Download it again later.",
    },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          messages: {
            en: {
              shared: {
                binaryDownload: {
                  installedVersion: "Installed version",
                  availableVersion: "Latest available version",
                  versionNotInstalled: "Not installed",
                  versionUnknown: "Unknown",
                },
              },
            },
          },
        }),
      ],
      stubs: {
        Button: button,
        Popover: popover,
        PopoverContent: passthrough,
        PopoverTrigger: passthrough,
        Progress: true,
        ResourceStatusCard: resourceCard,
        Skeleton: true,
      },
    },
  });

describe("BinaryDownloadCard Cloudflared version states", () => {
  it("renders the regular download action for a missing installation", async () => {
    const wrapper = mountCard("missing");

    expect(wrapper.findAll("dl dd").map((value) => value.text())).toEqual([
      "Not installed",
      "2026.11.2",
    ]);
    expect(
      wrapper.find('[data-testid="binary-outdated-warning"]').exists(),
    ).toBe(false);
    const action = wrapper
      .findAll("button")
      .find((candidate) => candidate.text() === "Download");
    expect(action).toBeDefined();
    await action?.trigger("click");
    expect(wrapper.emitted("start")).toHaveLength(1);
  });

  it("shows an outdated warning and confirms the update action", async () => {
    const wrapper = mountCard("outdated", "2026.9.1");

    expect(wrapper.findAll("dl dt").map((label) => label.text())).toEqual([
      "Installed version",
      "Latest available version",
    ]);
    expect(wrapper.findAll("dl dd").map((value) => value.text())).toEqual([
      "2026.9.1",
      "2026.11.2",
    ]);
    expect(
      wrapper.get('[data-testid="binary-outdated-warning"]').text(),
    ).toContain("The tunnel reconnects during the update.");
    expect(wrapper.text()).toContain("The tunnel reconnects briefly.");
    const updateActions = wrapper
      .findAll("button")
      .filter((candidate) => candidate.text() === "Update now");
    expect(updateActions.length).toBeGreaterThanOrEqual(2);
    await updateActions.at(-1)?.trigger("click");
    expect(wrapper.emitted("update")).toHaveLength(1);
  });

  it("updates the installed version when installation completes", async () => {
    const wrapper = mountCard("outdated");
    expect(wrapper.get("dl dd").text()).toBe("Unknown");
    await wrapper.setProps({
      installedVersion: "2026.11.2",
      installationStatus: "current",
      downloaded: true,
    });

    expect(wrapper.findAll("dl dd").map((value) => value.text())).toEqual([
      "2026.11.2",
      "2026.11.2",
    ]);
    expect(wrapper.text()).toContain("Ready");
    expect(
      wrapper.find('[data-testid="binary-outdated-warning"]').exists(),
    ).toBe(false);
    expect(wrapper.text()).not.toContain("Update now");
  });
});
