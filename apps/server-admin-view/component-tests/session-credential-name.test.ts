import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { defineComponent, h, ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const isTouchInteraction = ref(true);

vi.mock("@admin-shared/composables/useMediaQueryMatch", () => ({
  useMediaQueryMatch: () => isTouchInteraction,
}));

import SessionCredentialName from "../src/views/session-management/SessionCredentialName.vue";

const PassThrough = defineComponent({
  setup(_, { slots }) {
    return () => h("div", slots.default?.());
  },
});

const TooltipStub = defineComponent({
  name: "Tooltip",
  props: { open: Boolean },
  emits: ["update:open"],
  setup(_, { slots }) {
    return () => h("div", { "data-tooltip": "" }, slots.default?.());
  },
});

const mountCredential = () => {
  const i18n = createI18n({
    legacy: false,
    locale: "zh-CN",
    messages: {
      "zh-CN": {
        admin: {
          sessions: {
            credentialMethods: {
              totp: "TOTP",
              passkey: "Passkey",
              password: "密码",
              oidc: "OIDC",
              ldap: "LDAP",
            },
            credentialDisplay: {
              methodWithCredential: "{method}：{name}",
              relation: "{parent} / {child}",
            },
          },
        },
      },
    },
  });

  return mount(SessionCredentialName, {
    props: {
      session: {
        method: "PASSKEY",
        credentialName: "macOS",
        linkedTotpName: "admin mac",
      },
    },
    global: {
      plugins: [i18n],
      stubs: {
        Tooltip: TooltipStub,
        TooltipProvider: PassThrough,
        TooltipTrigger: PassThrough,
        TooltipContent: PassThrough,
      },
    },
  });
};

describe("SessionCredentialName", () => {
  beforeEach(() => {
    isTouchInteraction.value = true;
  });

  it("shows the parent name and complete login relation", () => {
    const wrapper = mountCredential();
    expect(wrapper.get("button").text()).toBe("admin mac");
    expect(wrapper.text()).toContain("TOTP：admin mac / Passkey：macOS");
  });

  it("toggles on touch and closes when the tooltip requests it", async () => {
    const wrapper = mountCredential();
    const tooltip = wrapper.getComponent(TooltipStub);

    expect(tooltip.props("open")).toBe(false);
    await wrapper.get("button").trigger("click");
    expect(tooltip.props("open")).toBe(true);
    await wrapper.get("button").trigger("click");
    expect(tooltip.props("open")).toBe(false);

    await wrapper.get("button").trigger("click");
    tooltip.vm.$emit("update:open", false);
    await wrapper.vm.$nextTick();
    expect(tooltip.props("open")).toBe(false);
  });

  it("leaves desktop hover and focus behavior to the tooltip primitive", async () => {
    isTouchInteraction.value = false;
    const wrapper = mountCredential();
    const tooltip = wrapper.getComponent(TooltipStub);

    await wrapper.get("button").trigger("click");
    expect(tooltip.props("open")).toBe(false);
    tooltip.vm.$emit("update:open", true);
    await wrapper.vm.$nextTick();
    expect(tooltip.props("open")).toBe(true);
  });
});
