import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import DDNSUpdateProxyTest from "../src/views/ddns-management/DDNSUpdateProxyTest.vue";
import { createUpdateProxyDraft } from "../src/views/ddns-management/ddns-proxy-model";
import { DDNSAPI, type DDNSUpdateProxyTestResult } from "../src/lib/api/ddns";

const draft = () =>
  createUpdateProxyDraft({
    url: "http://localhost:7890",
    username: "user",
    passwordConfigured: true,
  });
const success: DDNSUpdateProxyTestResult = {
  success: true,
  elapsedMs: 42,
  status: 200,
  error: null,
  url: "https://www.cloudflare.com/cdn-cgi/trace",
};
const render = (value = draft()) =>
  mount(DDNSUpdateProxyTest, {
    props: { draft: value, transport: "curl", disabled: false },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          messages: { en: { admin: enAdmin } },
        }),
      ],
    },
  });
afterEach(() => vi.restoreAllMocks());

describe("DDNS proxy connectivity test", () => {
  it("requires a proxy address even with the default switch off", async () => {
    const request = vi
      .spyOn(DDNSAPI, "testUpdateProxy")
      .mockResolvedValue(success);
    const wrapper = render(createUpdateProxyDraft());
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
    await wrapper.get("button").trigger("click");
    expect(request).not.toHaveBeenCalled();
    await wrapper.setProps({ draft: draft() });
    expect(wrapper.get("button").attributes("disabled")).toBeUndefined();
    wrapper.unmount();
  });

  it("tests the unsaved draft once, keeps saved credentials write-only and never saves settings", async () => {
    let complete!: (result: DDNSUpdateProxyTestResult) => void;
    const request = vi.spyOn(DDNSAPI, "testUpdateProxy").mockImplementation(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const save = vi.spyOn(DDNSAPI, "saveSettings");
    const dnsUpdate = vi.spyOn(DDNSAPI, "test");
    const wrapper = render();
    await wrapper.get("button").trigger("click");
    await wrapper.get("button").trigger("click");
    expect(request).toHaveBeenCalledTimes(1);
    expect(request.mock.calls[0]![0]).toEqual({
      httpTransport: "curl",
      updateProxy: {
        enabled: false,
        url: "http://localhost:7890",
        username: "user",
        password: "",
        clearPassword: false,
      },
    });
    expect(wrapper.emitted("busy")).toEqual([[true]]);
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
    complete(success);
    await flushPromises();
    expect(wrapper.get('[role="status"]').text()).toContain("42 ms");
    expect(wrapper.emitted("busy")).toEqual([[true], [false]]);
    expect(save).not.toHaveBeenCalled();
    expect(dnsUpdate).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("preserves failed input, supports replacement and clearing, and discards outdated results", async () => {
    const request = vi
      .spyOn(DDNSAPI, "testUpdateProxy")
      .mockResolvedValue({
        ...success,
        success: false,
        error: "Proxy authentication failed",
        status: 407,
      });
    const wrapper = render({ ...draft(), password: "replacement" });
    await wrapper.get("button").trigger("click");
    await flushPromises();
    expect(request.mock.calls[0]![0].updateProxy.password).toBe("replacement");
    expect(wrapper.get('[role="alert"]').text()).toContain(
      "authentication failed",
    );
    expect(wrapper.props("draft").password).toBe("replacement");
    await wrapper.setProps({ draft: { ...draft(), clearPassword: true } });
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    let complete!: (result: DDNSUpdateProxyTestResult) => void;
    request.mockImplementation(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    await wrapper.get("button").trigger("click");
    expect(request.mock.calls[1]![0].updateProxy.clearPassword).toBe(true);
    const signal = request.mock.calls[1]![1]!;
    await wrapper.setProps({ transport: "node" });
    expect(signal.aborted).toBe(true);
    complete(success);
    await flushPromises();
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
    await wrapper.get("button").trigger("click");
    const finalSignal = request.mock.calls[2]![1]!;
    wrapper.unmount();
    expect(finalSignal.aborted).toBe(true);
    complete(success);
    await flushPromises();
  });
});
