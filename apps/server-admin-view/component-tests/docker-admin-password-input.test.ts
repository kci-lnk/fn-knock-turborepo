import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";

import DockerAdminPasswordInput from "../src/components/DockerAdminPasswordInput.vue";

function mountPasswordInput(disabled = false) {
  const i18n = createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        admin: {
          dockerAdmin: {
            hidePassword: "Hide password",
            showPassword: "Show password",
          },
        },
      },
    },
  });

  return mount(DockerAdminPasswordInput, {
    props: {
      disabled,
      id: "admin-password",
      modelValue: "initial-secret",
    },
    attrs: {
      autocomplete: "current-password",
    },
    global: { plugins: [i18n] },
  });
}

describe("DockerAdminPasswordInput", () => {
  it("toggles real password visibility and emits edited values", async () => {
    const wrapper = mountPasswordInput();
    const input = wrapper.get("input");
    const toggle = wrapper.get("button");

    expect(input.attributes("type")).toBe("password");
    expect(input.attributes("autocomplete")).toBe("current-password");
    expect(toggle.attributes("aria-label")).toBe("Show password");

    await toggle.trigger("click");
    expect(input.attributes("type")).toBe("text");
    expect(toggle.attributes("aria-label")).toBe("Hide password");

    await input.setValue("updated-secret");
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([
      "updated-secret",
    ]);

    await wrapper.setProps({ modelValue: "" });
    expect(input.attributes("type")).toBe("password");
  });

  it("disables both editing and visibility controls", () => {
    const wrapper = mountPasswordInput(true);
    expect(wrapper.get("input").attributes()).toHaveProperty("disabled");
    expect(wrapper.get("button").attributes()).toHaveProperty("disabled");
  });
});
