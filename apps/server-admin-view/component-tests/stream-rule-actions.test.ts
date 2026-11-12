import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import type { StreamMapping } from "../src/types";
import StreamMappingTable from "../src/views/stream-mappings/StreamMappingTable.vue";
import type { StreamRuleAction } from "../src/views/stream-mappings/streamMappingModel";

const rules: StreamMapping[] = [
  { protocol: "tcp", listen_port: 5050, use_auth: true, target: "host:50" },
  {
    protocol: "udp",
    listen_port: 5050,
    use_auth: true,
    target: "host:50",
    manually_disabled: true,
  },
  { protocol: "tcp", listen_port: 8080, use_auth: true, target: "web:80" },
];

function mountTable() {
  const save = vi
    .fn<
      (
        action: StreamRuleAction,
        rules: readonly StreamMapping[],
      ) => Promise<boolean>
    >()
    .mockResolvedValue(true);
  const wrapper = mount(StreamMappingTable, {
    attachTo: document.body,
    props: {
      mappings: rules,
      onRuleAction: save,
      trafficTimestamp: null,
      probingMappingKey: null,
      getStreamTrafficSample: () => null,
      onSaveComment: async () => {},
    },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          messages: {
            en: {
              admin: enAdmin,
              common: { cancel: "Cancel", moreActions: "More actions" },
            },
          },
        }),
      ],
      stubs: { InlineCommentEditor: true, StreamTrafficActivity: true },
    },
  });
  afterEachCleanup = () => wrapper.unmount();
  return { wrapper, save };
}
let afterEachCleanup: () => void;
afterEach(() => {
  afterEachCleanup();
  document.body.innerHTML = "";
});

function button(parent: ParentNode, text: string) {
  const element = [...parent.querySelectorAll("button")].find(
    (item) => item.textContent?.trim() === text,
  );
  if (!element) throw new Error(`Button not found: ${text}`);
  return element;
}
async function click(element: HTMLElement) {
  element.click();
  await flushPromises();
}
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')!;

it("scopes select-all to search results and keeps selection available after cancellation or failure", async () => {
  const { wrapper, save } = mountTable();
  await wrapper.get("[aria-pressed]").trigger("click");
  await wrapper.get("input").setValue("5050");
  const checks = wrapper.findAll('[role="checkbox"]');
  expect(checks).toHaveLength(3);
  await checks[1]!.trigger("click");
  expect(checks[0]!.attributes("aria-checked")).toBe("mixed");
  await checks[0]!.trigger("click");
  const toolbar = wrapper.get('[role="toolbar"]').element;
  expect(toolbar.textContent).toContain("Selected rules: 2");
  await click(button(toolbar, "Disable"));
  expect(dialog().textContent).toContain("Disable selected rules (1)");
  await click(button(dialog(), "Cancel"));
  expect(save).not.toHaveBeenCalled();

  await click(button(toolbar, "Delete"));
  expect(dialog().textContent).toContain("selected rules (2)");
  let finish!: (success: boolean) => void;
  save.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  await click(button(dialog(), "Delete"));
  expect(button(dialog(), "Delete").disabled).toBe(true);
  await click(button(dialog(), "Delete"));
  expect(save).toHaveBeenCalledTimes(1);
  finish(false);
  await flushPromises();
  expect(toolbar.textContent).toContain("Selected rules: 2");
  await click(button(dialog(), "Delete"));
  expect(save).toHaveBeenLastCalledWith("delete", rules.slice(0, 2));
  expect(toolbar.textContent).toContain("Selected rules: 0");

  await checks[0]!.trigger("click");
  await wrapper.get("input").setValue("8080");
  expect(toolbar.textContent).toContain("Selected rules: 0");
  await wrapper.get('[role="checkbox"]').trigger("click");
  await wrapper.get("[aria-pressed]").trigger("click");
  await wrapper.get("[aria-pressed]").trigger("click");
  expect(wrapper.get('[role="toolbar"]').text()).toContain("Selected rules: 0");
});

it("confirms a single disabled UDP rule by protocol and port", async () => {
  const { wrapper, save } = mountTable();
  const row = wrapper.findAll("tbody tr")[1]!;
  expect(row.text()).toContain("Disabled");
  await row
    .get('button[aria-label="More actions"]')
    .trigger("keydown", { key: "Enter" });
  await flushPromises();
  const item = [
    ...document.querySelectorAll<HTMLElement>('[role="menuitem"]'),
  ].find((element) => element.textContent?.trim() === "Enable");
  expect(item).toBeDefined();
  await click(item!);
  expect(dialog().textContent).toContain("Enable UDP/5050?");
  await click(button(dialog(), "Enable"));
  expect(save).toHaveBeenCalledExactlyOnceWith("enable", [rules[1]]);
});

it("dismisses a stale delete confirmation when a failed save reloads a replacement target", async () => {
  const { wrapper, save } = mountTable();
  await wrapper.get("[aria-pressed]").trigger("click");
  await wrapper.findAll('[role="checkbox"]')[1]!.trigger("click");
  await click(button(wrapper.get('[role="toolbar"]').element, "Delete"));
  save.mockImplementationOnce(async () => {
    await wrapper.setProps({
      mappings: [{ ...rules[0]!, target: "replacement:80" }, ...rules.slice(1)],
    });
    return false;
  });
  await click(button(dialog(), "Delete"));
  expect(dialog()).toBeNull();
  expect(wrapper.get('[role="toolbar"]').text()).toContain("Selected rules: 0");
  expect(save).toHaveBeenCalledTimes(1);
});
