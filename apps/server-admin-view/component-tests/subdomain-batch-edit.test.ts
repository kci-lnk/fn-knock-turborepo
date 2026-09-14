import { mount, flushPromises } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { computed, nextTick, ref } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { useSubdomainBatchEdit } from "../src/views/subdomain-proxy/useSubdomainBatchEdit";
import SubdomainBatchEditDialog from "../src/views/subdomain-proxy/SubdomainBatchEditDialog.vue";
import SubdomainMappingsBatchActions from "../src/views/subdomain-proxy/SubdomainMappingsBatchActions.vue";
import { enAdmin as en } from "../../../packages/i18n/src/messages/admin/en";

const i18n = () =>
  createI18n({ legacy: false, locale: "en", messages: { en: { admin: en } } });
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  wrappers.forEach((wrapper) => wrapper.unmount());
  wrappers.length = 0;
  document.body.innerHTML = "";
});
const button = (text: string) =>
  [...document.querySelectorAll("button")].find(
    (item) => item.textContent?.trim() === text,
  )!;
const setup = () => {
  const mappings = ref(
    ["one.test", "two.test"].map((host) => ({
      ...createDefaultMapping(),
      host,
      target: "http://backend:8080",
      title: "Auto",
    })),
  );
  let saves = 0;
  const editor = useSubdomainBatchEdit({
    allMappings: computed(() => mappings.value),
    isSavingMappings: ref(false),
    isWindows: () => false,
    isAuthServiceTarget: () => false,
    saveHostMappings: async (next) => {
      saves++;
      mappings.value = next;
    },
    reloadMappings: async () => {},
    readGatewayHosts: async () => [],
    writeGatewayHosts: async () => {},
    translate: (key) => key,
    onSaved: () => {},
  });
  editor.openDialog(["one.test", "two.test"], () => {});
  const wrapper = mount(SubdomainBatchEditDialog, {
    props: { controller: editor },
    attachTo: document.body,
    global: { plugins: [i18n()] },
  });
  wrappers.push(wrapper);
  return {
    editor,
    mappings,
    get saves() {
      return saves;
    },
  };
};
describe("batch mapping editor", () => {
  it("has labeled independent fields, responsive rows, scrollable content and a fixed footer", async () => {
    const ctx = setup();
    await flushPromises();
    const content = document.querySelector('[data-slot="dialog-content"]')!;
    expect(content.className).toContain("flex-col");
    expect(content.className).toContain("overflow-hidden");
    expect(content.querySelectorAll("fieldset")).toHaveLength(2);
    expect(content.querySelectorAll("input")).toHaveLength(6);
    expect(content.querySelector(".md\\:grid-cols-3")).not.toBeNull();
    expect(content.querySelector(".overflow-y-auto")).not.toBeNull();
    for (const input of content.querySelectorAll("input"))
      expect(content.querySelector(`label[for="${input.id}"]`)).not.toBeNull();
    expect(content.textContent).toContain("Fetched title: Auto");
    expect(button("Save all").disabled).toBe(true);
    const title = document.querySelector<HTMLInputElement>("#batch-title-0")!;
    title.value = "Renamed app";
    title.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    expect(ctx.mappings.value[0]!.title_override).toBe("");
    expect(ctx.editor.rows.value[1]!.title).toBe("");
    button("Save all").click();
    await flushPromises();
    expect(ctx.saves).toBe(1);
    expect(ctx.mappings.value[0]!.title_override).toBe("Renamed app");
  });
  it("focuses the first invalid field and preserves unsaved edits when declining close", async () => {
    const ctx = setup();
    await flushPromises();
    ctx.editor.rows.value[0]!.host = "bad host";
    await nextTick();
    button("Save all").click();
    await flushPromises();
    expect(ctx.saves).toBe(0);
    expect(document.activeElement?.id).toBe("batch-host-0");
    expect(
      document.querySelector("#batch-host-0")?.getAttribute("aria-describedby"),
    ).toBe("batch-host-error-0");
    button("Cancel").click();
    await flushPromises();
    expect(ctx.editor.discardOpen.value).toBe(true);
    button("Continue editing").click();
    await flushPromises();
    expect(ctx.editor.open.value).toBe(true);
    expect(ctx.editor.rows.value[0]!.host).toBe("bad host");
    button("Cancel").click();
    await flushPromises();
    button("Close and discard").click();
    await flushPromises();
    expect(ctx.editor.open.value).toBe(false);
  });
  it("moves actions into a keyboard-accessible dropdown and emits edit", async () => {
    const wrapper = mount(SubdomainMappingsBatchActions, {
      props: { groups: [], saving: false, selectedCount: 2 },
      attachTo: document.body,
      global: { plugins: [i18n()] },
    });
    wrappers.push(wrapper);
    expect(wrapper.findAll("button")).toHaveLength(2);
    const trigger = wrapper.find('button[aria-haspopup="menu"]');
    await trigger.trigger("keydown", { key: "Enter" });
    await flushPromises();
    const items = [...document.querySelectorAll('[role="menuitem"]')];
    expect(items.map((item) => item.textContent)).toEqual(
      expect.arrayContaining([
        expect.stringContaining("Edit titles / domains / targets"),
        expect.stringContaining("Delete"),
      ]),
    );
    (
      items.find((item) =>
        item.textContent?.includes("Edit titles"),
      ) as HTMLElement
    ).click();
    await flushPromises();
    expect(wrapper.emitted("edit")).toHaveLength(1);
  });
  it("preserves group moves through the submenu and disables actions while saving", async () => {
    const wrapper = mount(SubdomainMappingsBatchActions, {
      props: {
        groups: [{ id: "internal", name: "Internal" }],
        saving: false,
        selectedCount: 2,
      },
      attachTo: document.body,
      global: { plugins: [i18n()] },
    });
    wrappers.push(wrapper);
    await wrapper
      .find('button[aria-haspopup="menu"]')
      .trigger("keydown", { key: "Enter" });
    await flushPromises();
    const groupTrigger = [
      ...document.querySelectorAll<HTMLElement>('[role="menuitem"]'),
    ].find((item) =>
      item.textContent?.includes(en.subdomainProxy.moveToGroup),
    )!;
    groupTrigger.focus();
    groupTrigger.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
    );
    await flushPromises();
    const group = [
      ...document.querySelectorAll<HTMLElement>('[role="menuitem"]'),
    ].find((item) => item.textContent?.trim() === "Internal")!;
    group.click();
    await flushPromises();
    expect(wrapper.emitted("move")).toEqual([["internal"]]);
    await wrapper.setProps({ saving: true });
    expect(
      wrapper.find('button[aria-haspopup="menu"]').attributes(),
    ).toHaveProperty("disabled");
  });
});
