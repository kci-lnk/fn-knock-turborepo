import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { afterEach, describe, expect, it } from "vitest";

import { usePollingResourceStatus } from "@admin-shared/composables/usePollingResourceStatus";

const setDocumentHidden = (hidden: boolean) => {
  Object.defineProperty(document, "hidden", {
    configurable: true,
    value: hidden,
  });
  document.dispatchEvent(new Event("visibilitychange"));
};

describe("usePollingResourceStatus", () => {
  afterEach(() => setDocumentHidden(false));

  it("resumes after visibility aborts the initial request", async () => {
    let calls = 0;
    const component = defineComponent({
      setup() {
        usePollingResourceStatus({
          intervalMs: 60_000,
          fetcher: (signal) => {
            calls += 1;
            if (calls > 1) return Promise.resolve({ downloading: false });
            return new Promise<{ downloading: boolean }>((_, reject) => {
              signal?.addEventListener(
                "abort",
                () => reject(new DOMException("Aborted", "AbortError")),
                { once: true },
              );
            });
          },
          onData: () => undefined,
          isDownloading: (data) => data.downloading,
        });
        return () => h("div");
      },
    });

    const wrapper = mount(component);
    await new Promise((resolve) => setTimeout(resolve, 5));
    expect(calls).toBe(1);

    setDocumentHidden(true);
    await new Promise((resolve) => setTimeout(resolve, 5));
    setDocumentHidden(false);
    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(calls).toBe(2);
    wrapper.unmount();
  });
});
