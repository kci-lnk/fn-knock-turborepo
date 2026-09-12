import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { afterEach, expect, it, vi } from "vitest";
import ActionTooltip from "../src/components/ActionTooltip.vue";
import ConfirmDangerTooltip from "../src/components/ConfirmDangerTooltip.vue";
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
afterEach(() => {
  document.body.innerHTML = "";
});
it("shows a focus hint without preventing the confirmation or executing it early", async () => {
  const confirm = vi.fn();
  const wrapper = mount(
    defineComponent({
      components: { ConfirmDangerTooltip },
      setup: () => ({ confirm }),
      template: `<ConfirmDangerTooltip label="Block IP" title="Confirm block" description="Block this address" :on-confirm="confirm"><template #trigger><button>Block</button></template></ConfirmDangerTooltip>`,
    }),
    { attachTo: document.body },
  );
  wrapper.get("button").element.focus();
  await flushPromises();
  expect(document.body.textContent).toContain("Block IP");
  await wrapper.get("button").trigger("click");
  await flushPromises();
  expect(document.body.textContent).toContain("Confirm block");
  expect(confirm).not.toHaveBeenCalled();
  const apply = Array.from(document.querySelectorAll("button")).find(
    (button) => button.textContent?.trim() === "common.confirmDelete",
  );
  expect(apply).toBeDefined();
  apply!.click();
  await flushPromises();
  expect(confirm).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});

it("associates the tooltip with the actual focused action", async () => {
  const wrapper = mount(ActionTooltip, {
    props: { label: "Open details" },
    slots: { default: '<button aria-label="Details">Details</button>' },
    attachTo: document.body,
  });
  wrapper.get("button").element.focus();
  await flushPromises();
  const descriptionId = wrapper.get("button").attributes("aria-describedby");
  expect(descriptionId).toBeTruthy();
  expect(document.getElementById(descriptionId!)?.textContent).toContain(
    "Open details",
  );
  wrapper.unmount();
});
