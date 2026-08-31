import { defineComponent, h, nextTick, ref } from "vue";
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import {
  Select,
  SelectTrigger,
} from "@/components/ui/select";

const SelectHarness = defineComponent({
  setup() {
    const open = ref(false);
    return () =>
      h("div", [
        h(
          Select,
          {
            open: open.value,
            "onUpdate:open": (value: boolean) => {
              open.value = value;
            },
          },
          {
            default: () =>
              h(
                SelectTrigger,
                { "data-testid": "trigger" },
                () => "Response type",
              ),
          },
        ),
        h("output", { "data-testid": "open-state" }, String(open.value)),
      ]);
  },
});

const dispatchPointer = (
  element: Element,
  type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
  init: PointerEventInit,
) => {
  const pointerId = init.pointerId ?? 1;
  element.dispatchEvent(
    new PointerEvent(type, {
      bubbles: true,
      cancelable: true,
      pointerId,
      ...init,
    }),
  );
  // happy-dom does not always perform the browser's automatic implicit
  // pointer-capture release when a capture listener stops pointerup.
  if (
    (type === "pointerup" || type === "pointercancel") &&
    element instanceof HTMLElement &&
    element.hasPointerCapture?.(pointerId)
  ) {
    element.releasePointerCapture(pointerId);
  }
};

const touchLikeMouse = {
  pointerType: "mouse",
  width: 16,
  height: 16,
};

describe("SelectTrigger touch scroll guard", () => {
  it("keeps an intentional touch tap able to open the select", async () => {
    const wrapper = mount(SelectHarness, { attachTo: document.body });
    const trigger = wrapper.get('[data-testid="trigger"]');

    dispatchPointer(trigger.element, "pointerdown", {
      pointerId: 12,
      pointerType: "touch",
      clientX: 40,
      clientY: 100,
    });
    dispatchPointer(trigger.element, "pointermove", {
      pointerId: 12,
      pointerType: "touch",
      clientX: 43,
      clientY: 104,
    });
    dispatchPointer(trigger.element, "pointerup", {
      pointerId: 12,
      pointerType: "touch",
      clientX: 43,
      clientY: 104,
    });
    await nextTick();

    expect(wrapper.get('[data-testid="open-state"]').text()).toBe("true");
    wrapper.unmount();
  });

  it("does not open after a touch gesture moves beyond the scroll threshold", async () => {
    const wrapper = mount(SelectHarness, { attachTo: document.body });
    const trigger = wrapper.get('[data-testid="trigger"]');

    dispatchPointer(trigger.element, "pointerdown", {
      pointerId: 11,
      pointerType: "touch",
      clientX: 40,
      clientY: 100,
    });
    dispatchPointer(trigger.element, "pointermove", {
      pointerId: 11,
      pointerType: "touch",
      clientX: 40,
      clientY: 124,
    });
    dispatchPointer(trigger.element, "pointerup", {
      pointerId: 11,
      pointerType: "touch",
      clientX: 40,
      clientY: 124,
    });
    await nextTick();

    expect(wrapper.get('[data-testid="open-state"]').text()).toBe("false");
    wrapper.unmount();
  });

  it("replays a touch-like mouse tap as an intentional touch activation", async () => {
    const wrapper = mount(SelectHarness, { attachTo: document.body });
    const trigger = wrapper.get('[data-testid="trigger"]');

    dispatchPointer(trigger.element, "pointerdown", {
      ...touchLikeMouse,
      pointerId: 14,
      clientX: 40,
      clientY: 100,
    });
    dispatchPointer(trigger.element, "pointerup", {
      ...touchLikeMouse,
      pointerId: 14,
      clientX: 42,
      clientY: 103,
    });
    await nextTick();

    expect(wrapper.get('[data-testid="open-state"]').text()).toBe("true");
    wrapper.unmount();
  });

  it("also suppresses scrolling when a mobile WebView reports touch as mouse", async () => {
    const wrapper = mount(SelectHarness, { attachTo: document.body });
    const trigger = wrapper.get('[data-testid="trigger"]');

    dispatchPointer(trigger.element, "pointerdown", {
      ...touchLikeMouse,
      pointerId: 13,
      clientX: 40,
      clientY: 100,
    });
    await nextTick();
    expect(wrapper.get('[data-testid="open-state"]').text()).toBe("false");

    // Some embedded browsers coalesce the move and expose only final pointerup
    // coordinates, so the release position must also be guarded.
    dispatchPointer(trigger.element, "pointerup", {
      ...touchLikeMouse,
      pointerId: 13,
      clientX: 40,
      clientY: 128,
    });
    await nextTick();

    expect(wrapper.get('[data-testid="open-state"]').text()).toBe("false");
    wrapper.unmount();
  });
});
