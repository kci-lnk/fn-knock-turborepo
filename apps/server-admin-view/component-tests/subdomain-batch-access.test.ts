import { mount, flushPromises } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it } from "vitest";
import SubdomainMappingsBatchActions from "../src/views/subdomain-proxy/SubdomainMappingsBatchActions.vue";
import SubdomainActionConfirmDialog from "../src/views/subdomain-proxy/SubdomainActionConfirmDialog.vue";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  wrappers.forEach((wrapper) => wrapper.unmount());
  wrappers.length = 0;
  document.body.innerHTML = "";
});
const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: { admin: enAdmin, common: { moreActions: "More actions" } },
    },
  });

describe("batch access actions", () => {
  for (const action of ["require-auth", "public-access"] as const) {
    it(`emits ${action} through the shared toolbar`, async () => {
      const wrapper = mount(SubdomainMappingsBatchActions, {
        props: { groups: [], saving: false, selectedCount: 2 },
        attachTo: document.body,
        global: { plugins: [i18n()] },
      });
      wrappers.push(wrapper);
      await wrapper
        .get('[data-testid="batch-more-trigger"]')
        .trigger("keydown", { key: "Enter" });
      await flushPromises();
      const item = document.querySelector<HTMLElement>(
        `[data-testid="batch-${action}"]`,
      )!;
      expect(item.textContent).toContain(
        action === "require-auth"
          ? enAdmin.subdomainProxy.authRequired
          : enAdmin.subdomainProxy.publicAccess,
      );
      item.click();
      await flushPromises();
      expect(wrapper.emitted(action)).toEqual([[]]);
      await wrapper.setProps({ saving: true });
      expect(
        wrapper.get('[data-testid="batch-more-trigger"]').attributes(),
      ).toHaveProperty("disabled");
      await wrapper.setProps({ saving: false, selectedCount: 0 });
      expect(
        wrapper.get('[data-testid="batch-more-trigger"]').attributes(),
      ).toHaveProperty("disabled");
    });
  }

  it("renders the access description and blocks all dismissal while saving", async () => {
    const wrapper = mount(SubdomainActionConfirmDialog, {
      props: {
        open: true,
        loading: true,
        title: "Allow public access to 2 subdomains?",
        description: enAdmin.subdomainProxy.batchPublicAccessDescription,
        cancelLabel: "Cancel",
        confirmLabel: "Allow public access (2)",
        confirmVariant: "destructive",
      },
      attachTo: document.body,
    });
    wrappers.push(wrapper);
    await flushPromises();
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(dialog.textContent).toContain(
      "Independent path authentication rules still apply",
    );
    const buttons = [...dialog.querySelectorAll("button")];
    expect(buttons).toHaveLength(2);
    expect(buttons.every((button) => button.disabled)).toBe(true);
    buttons.forEach((button) => button.click());
    dialog.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Escape",
        bubbles: true,
        cancelable: true,
      }),
    );
    await flushPromises();
    expect(wrapper.emitted("update:open")).toBeUndefined();
    expect(wrapper.emitted("cancel")).toBeUndefined();
    expect(wrapper.emitted("confirm")).toBeUndefined();
    await wrapper.setProps({ loading: false });
    buttons[0]!.click();
    buttons[1]!.click();
    expect(wrapper.emitted("cancel")).toHaveLength(1);
    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });
});
