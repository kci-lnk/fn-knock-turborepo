import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { describe, expect, it } from "vitest";

import { Input } from "@/components/ui/input";

const ignoreAttributes = [
  "data-form-type",
  "data-1p-ignore",
  "data-lpignore",
  "data-bwignore",
];

describe("Input password-manager support", () => {
  it("preserves passive updates and external resets for ordinary inputs", async () => {
    const wrapper = mount(Input, { props: { modelValue: "initial" } });
    const field = wrapper.get("input");
    field.element.value = "edited";
    field.element.dispatchEvent(new Event("input", { bubbles: true }));
    expect(wrapper.emitted("update:modelValue")).toBeUndefined();

    await nextTick();
    expect(wrapper.emitted("update:modelValue")).toEqual([["edited"]]);
    await wrapper.setProps({ modelValue: "reset" });
    expect(field.element.value).toBe("reset");
    expect(wrapper.emitted("update:modelValue")).toEqual([["edited"]]);
    wrapper.unmount();
  });

  it("emits autofill synchronously once and retains support for unbound inputs", async () => {
    const wrapper = mount(Input, {
      props: { allowPasswordManager: true, defaultValue: "initial" },
      attrs: { autocomplete: "username" },
    });
    const field = wrapper.get("input");
    field.element.value = "alice";
    field.element.dispatchEvent(new Event("input", { bubbles: true }));
    expect(wrapper.emitted("update:modelValue")).toEqual([["alice"]]);

    await nextTick();
    expect(wrapper.emitted("update:modelValue")).toEqual([["alice"]]);
    expect(field.element.value).toBe("alice");

    await wrapper.setProps({ modelValue: "" });
    expect(field.element.value).toBe("");
    expect(wrapper.emitted("update:modelValue")).toEqual([["alice"]]);
    wrapper.unmount();
  });

  it.each(["text", "password"])(
    "keeps autofill disabled by default for %s inputs",
    (type) => {
      const wrapper = mount(Input, { attrs: { type } });

      expect(wrapper.attributes("autocomplete")).toBe(
        type === "password" ? "new-password" : "off",
      );
      expect(wrapper.attributes("data-form-type")).toBe("other");
      for (const attribute of ignoreAttributes.slice(1)) {
        expect(wrapper.attributes(attribute)).toBe("true");
      }
      wrapper.unmount();
    },
  );

  it("removes ignore attributes entirely when opted in and restores them when disabled", async () => {
    const wrapper = mount(Input, {
      props: { allowPasswordManager: true },
      attrs: {
        type: "password",
        name: "password",
        autocomplete: "current-password",
      },
    });

    for (const attribute of ignoreAttributes) {
      expect(wrapper.element.hasAttribute(attribute)).toBe(false);
    }
    expect(wrapper.attributes("autocomplete")).toBe("current-password");
    expect(wrapper.attributes("name")).toBe("password");
    expect(wrapper.element.hasAttribute("allow-password-manager")).toBe(false);

    await wrapper.setProps({ allowPasswordManager: false });
    expect(wrapper.attributes("data-form-type")).toBe("other");
    for (const attribute of ignoreAttributes.slice(1)) {
      expect(wrapper.attributes(attribute)).toBe("true");
    }
    wrapper.unmount();
  });
});
