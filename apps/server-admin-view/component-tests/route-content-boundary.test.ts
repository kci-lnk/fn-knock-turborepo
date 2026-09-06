import { flushPromises, mount } from "@vue/test-utils";
import { defineAsyncComponent, defineComponent, h, onMounted } from "vue";
import {
  createMemoryHistory,
  createRouter,
  RouterView,
  useRoute,
} from "vue-router";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Boundary from "../src/views/layout/RouteContentBoundary.vue";
import {
  buildCacheBustedApplicationUrl,
  isDynamicImportFailure,
  replaceWithUpdatedApplication,
} from "../src/lib/update-reload";

vi.mock("../src/lib/update-reload", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../src/lib/update-reload")>()),
  replaceWithUpdatedApplication: vi.fn(),
}));
const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    missingWarn: false,
    fallbackWarn: false,
    messages: { en: {} },
  });
beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.restoreAllMocks());

describe("route content loading boundary", () => {
  it.each([
    "Failed to fetch dynamically imported module: /assets/old.js",
    "Unable to preload CSS for /assets/old.css",
  ])("shows manual recovery for %s", async (message) => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    let fail = true;
    const loader = vi.fn(async () => {
      if (fail) throw new TypeError(message);
      return defineComponent(() => () => h("p", "Recovered"));
    });
    const Child = defineAsyncComponent(loader);
    const wrapper = mount(Boundary, {
      props: { resetKey: "/mappings" },
      slots: { default: () => h(Child) },
      global: { plugins: [i18n()] },
    });
    try {
      await flushPromises();
      expect(wrapper.find('[role="alert"]').exists()).toBe(true);
      expect(loader).toHaveBeenCalledTimes(1);
      expect(replaceWithUpdatedApplication).not.toHaveBeenCalled();
      await wrapper.get("button").trigger("click");
      expect(replaceWithUpdatedApplication).toHaveBeenCalledExactlyOnceWith(
        "chunk",
      );
      fail = false;
      await wrapper.setProps({ resetKey: "/system?tab=maintenance" });
      await flushPromises();
      expect(wrapper.find('[role="alert"]').exists()).toBe(false);
      expect(wrapper.text()).toContain("Recovered");
    } finally {
      wrapper.unmount();
    }
  });

  it.each([
    new TypeError("Failed to fetch"),
    new TypeError("Load failed"),
    new Error("Business failure"),
  ])("lets ordinary errors propagate: %s", async (error) => {
    const handler = vi.fn();
    const Child = defineComponent({
      setup() {
        return () =>
          h(
            "button",
            {
              onClick: () => {
                throw error;
              },
            },
            "Run",
          );
      },
    });
    const wrapper = mount(Boundary, {
      props: { resetKey: "/mappings" },
      slots: { default: () => h(Child) },
      global: { plugins: [i18n()], config: { errorHandler: handler } },
    });
    try {
      await wrapper.get("button").trigger("click");
      await flushPromises();
      expect(handler.mock.calls[0]?.[0]).toBe(error);
      expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    } finally {
      wrapper.unmount();
    }
  });

  it("does not remount healthy content when the URL changes", async () => {
    const mounted = vi.fn();
    const Child = defineComponent({
      setup() {
        onMounted(mounted);
        return () => h("p", "Content");
      },
    });
    const wrapper = mount(Boundary, {
      props: { resetKey: "/mappings" },
      slots: { default: () => h(Child) },
      global: { plugins: [i18n()] },
    });
    try {
      await wrapper.setProps({ resetKey: "/mappings?tab=protocol" });
      expect(mounted).toHaveBeenCalledTimes(1);
    } finally {
      wrapper.unmount();
    }
  });

  it("recovers through real RouterView navigation and preserves healthy page instances", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const mounted = vi.fn();
    const Broken = defineAsyncComponent(async () => {
      throw new Error("Unable to preload CSS for /assets/mapping.css");
    });
    const Mapping = defineComponent({
      setup() {
        const route = useRoute();
        onMounted(mounted);
        return () =>
          route.query.tab === "healthy" ? h("p", "Mapping ready") : h(Broken);
      },
    });
    const Layout = defineComponent({
      setup() {
        const route = useRoute();
        return () =>
          h("main", [
            h("nav", "Navigation retained"),
            h(Boundary, { resetKey: route.fullPath }),
          ]);
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: "/",
          component: Layout,
          children: [
            { path: "mapping", component: Mapping },
            {
              path: "other",
              component: { render: () => h("p", "Other page") },
            },
            { path: "blocked", component: Mapping, beforeEnter: () => false },
          ],
        },
      ],
    });
    await router.push("/mapping");
    await router.isReady();
    const wrapper = mount(RouterView, {
      global: { plugins: [i18n(), router] },
    });
    try {
      await flushPromises();
      expect(wrapper.find('[role="alert"]').exists()).toBe(true);
      expect(wrapper.get("nav").text()).toBe("Navigation retained");
      await router.push("/blocked");
      await flushPromises();
      expect(wrapper.find('[role="alert"]').exists()).toBe(true);
      await router.push("/mapping?tab=healthy");
      await flushPromises();
      expect(wrapper.text()).toContain("Mapping ready");
      expect(wrapper.find('[role="alert"]').exists()).toBe(false);
      const mountCount = mounted.mock.calls.length;
      await router.push("/mapping?tab=healthy&filter=1");
      await flushPromises();
      expect(mounted).toHaveBeenCalledTimes(mountCount);
      await router.push("/mapping?tab=broken");
      await flushPromises();
      expect(wrapper.find('[role="alert"]').exists()).toBe(true);
      await router.push("/other");
      await flushPromises();
      expect(wrapper.text()).toContain("Other page");
      expect(wrapper.find('[role="alert"]').exists()).toBe(false);
      expect(replaceWithUpdatedApplication).not.toHaveBeenCalled();
    } finally {
      wrapper.unmount();
    }
  });

  it("ignores late import rejection after leaving its page", async () => {
    const failure = new Error(
      "Failed to fetch dynamically imported module: /old.js",
    );
    let reject!: (error: Error) => void;
    const Pending = defineAsyncComponent(
      () =>
        new Promise((_resolve, rejectLoad) => {
          reject = rejectLoad;
        }),
    );
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: "/",
          component: defineComponent({
            setup() {
              const route = useRoute();
              return () => h(Boundary, { resetKey: route.fullPath });
            },
          }),
          children: [
            { path: "pending", component: { render: () => h(Pending) } },
            { path: "ready", component: { render: () => h("p", "Ready") } },
          ],
        },
      ],
    });
    await router.push("/pending");
    const wrapper = mount(RouterView, {
      global: { plugins: [i18n(), router] },
    });
    try {
      await flushPromises();
      await router.push("/ready");
      await flushPromises();
      reject(failure);
      await flushPromises();
      expect(wrapper.text()).toContain("Ready");
      expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    } finally {
      wrapper.unmount();
    }
  });

  it("recognizes explicit module errors without changing existing generic error handling", () => {
    for (const message of [
      "Importing a module script failed.",
      "error loading dynamically imported module",
      "Load failed for module with source: dependency.js",
      "ChunkLoadError",
    ])
      expect(isDynamicImportFailure(new Error(message), false)).toBe(true);
    expect(isDynamicImportFailure(new TypeError("Failed to fetch"))).toBe(true);
    const url = new URL(
      buildCacheBustedApplicationUrl(
        "http://nas.test/cgi/ThirdParty/fn-knock/index.cgi/?foo=bar#/mappings?tab=protocol",
        123,
        "chunk",
      ),
    );
    expect(url.pathname).toBe("/cgi/ThirdParty/fn-knock/index.cgi/");
    expect(url.searchParams.get("foo")).toBe("bar");
    expect(url.searchParams.get("_fn_knock_reload")).toBe("123");
    expect(url.hash).toBe("#/mappings?tab=protocol");
  });
});
