import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import LayoutScrollArea from "../src/views/layout/LayoutScrollArea.vue";

const setScrollMetrics = (
  element: HTMLElement,
  metrics: { clientHeight: number; scrollHeight: number; scrollTop?: number },
) => {
  Object.defineProperties(element, {
    clientHeight: { configurable: true, value: metrics.clientHeight },
    scrollHeight: { configurable: true, value: metrics.scrollHeight },
    scrollTop: {
      configurable: true,
      writable: true,
      value: metrics.scrollTop ?? 0,
    },
  });
};

describe("LayoutScrollArea", () => {
  beforeEach(() => vi.useFakeTimers());

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("reveals a proportional indicator when overflowing content mounts", async () => {
    const wrapper = mount(LayoutScrollArea, {
      props: { hintOnMount: true },
      slots: { default: "<button>One</button><button>Two</button>" },
    });
    const viewport = wrapper.get(".layout-scroll-area__viewport");
    setScrollMetrics(viewport.element as HTMLElement, {
      clientHeight: 200,
      scrollHeight: 500,
    });

    window.dispatchEvent(new Event("resize"));
    await nextTick();

    const rail = wrapper.get(".layout-scroll-area__rail");
    expect(rail.classes()).toContain("layout-scroll-area__rail--visible");
    expect(wrapper.get(".layout-scroll-area__thumb").attributes("style")).toContain(
      "height: 77px",
    );

    vi.advanceTimersByTime(1600);
    await nextTick();
    expect(rail.classes()).not.toContain("layout-scroll-area__rail--visible");
    wrapper.unmount();
  });

  it("shows while scrolling and follows the current scroll position", async () => {
    const wrapper = mount(LayoutScrollArea);
    const viewport = wrapper.get(".layout-scroll-area__viewport");
    setScrollMetrics(viewport.element as HTMLElement, {
      clientHeight: 200,
      scrollHeight: 500,
      scrollTop: 150,
    });

    await viewport.trigger("scroll");

    expect(wrapper.get(".layout-scroll-area__rail").classes()).toContain(
      "layout-scroll-area__rail--visible",
    );
    expect(wrapper.get(".layout-scroll-area__thumb").attributes("style")).toContain(
      "translate3d(0, 58px, 0)",
    );
    wrapper.unmount();
  });

  it("does not render a misleading indicator without overflow", async () => {
    const wrapper = mount(LayoutScrollArea, {
      props: { hintOnMount: true },
    });
    const viewport = wrapper.get(".layout-scroll-area__viewport");
    setScrollMetrics(viewport.element as HTMLElement, {
      clientHeight: 200,
      scrollHeight: 200,
    });

    window.dispatchEvent(new Event("resize"));
    await nextTick();

    expect(wrapper.find(".layout-scroll-area__rail").exists()).toBe(false);
    wrapper.unmount();
  });
});
