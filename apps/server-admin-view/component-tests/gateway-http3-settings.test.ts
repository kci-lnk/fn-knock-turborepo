import { mount, flushPromises } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import GatewayHttp3Settings from "../src/views/system-settings/GatewayHttp3Settings.vue";
import { gatewayHttp3Api } from "../src/lib/api/gateway-http3";
import { toast } from "@admin-shared/utils/toast";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

const initial = {
  enabled: false,
  advertised_port: 0,
  state: "disabled",
  listen_addresses: [],
  error: "",
  active_connections: 0,
  handshake_failures: 0,
};
afterEach(() => vi.restoreAllMocks());
async function setup() {
  vi.spyOn(toast, "success").mockImplementation(() => undefined);
  vi.spyOn(gatewayHttp3Api, "get").mockResolvedValue({ ...initial });
  const wrapper = mount(GatewayHttp3Settings, {
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
  await flushPromises();
  return wrapper;
}
describe("HTTP/3 gateway settings", () => {
  it("defaults off and submits numeric advertised port", async () => {
    const update = vi.spyOn(gatewayHttp3Api, "set").mockResolvedValue({
      ...initial,
      enabled: true,
      advertised_port: 443,
      state: "running",
      listen_addresses: ["0.0.0.0:7999"],
    });
    const wrapper = await setup();
    expect(wrapper.get('[role="switch"]').attributes("aria-checked")).toBe(
      "false",
    );
    await wrapper.get('[role="switch"]').trigger("click");
    await wrapper.get('input[type="number"]').setValue("443");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Save")!
      .trigger("click");
    await flushPromises();
    expect(update).toHaveBeenCalledWith({
      enabled: true,
      advertised_port: 443,
    });
    expect(toast.success).toHaveBeenCalledWith("HTTP/3 settings saved");
    expect(wrapper.get('a[href="#/system?tab=gateway"]').text()).toBe(
      "Gateway",
    );
    expect(wrapper.text()).toContain("Listening locally");
    expect(wrapper.text()).toContain("only confirms the local listener");
    wrapper.unmount();
  });
  it("blocks invalid ports and preserves unsaved changes on failure", async () => {
    vi.spyOn(gatewayHttp3Api, "set").mockRejectedValue(
      new Error("UDP port occupied"),
    );
    const wrapper = await setup();
    await wrapper.get('[role="switch"]').trigger("click");
    await wrapper.get('input[type="number"]').setValue("65536");
    const save = wrapper
      .findAll("button")
      .find((button) => button.text() === "Save")!;
    expect(save.attributes("disabled")).toBeDefined();
    await wrapper.get('input[type="number"]').setValue("443");
    await save.trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain("UDP port occupied");
    expect(toast.success).not.toHaveBeenCalled();
    expect(wrapper.get('[role="switch"]').attributes("aria-checked")).toBe(
      "true",
    );
    wrapper.unmount();
  });
  it("refreshes status without discarding edits and resets only explicitly", async () => {
    const wrapper = await setup();
    await wrapper.get('[role="switch"]').trigger("click");
    await wrapper.get('input[type="number"]').setValue("8443");
    vi.mocked(gatewayHttp3Api.get).mockResolvedValue({
      ...initial,
      active_connections: 2,
    });
    const refresh = wrapper.get('button[aria-label="Refresh status"]');
    await refresh.trigger("click");
    await flushPromises();
    expect(wrapper.get('input[type="number"]').element.value).toBe("8443");
    expect(wrapper.get('[role="switch"]').attributes("aria-checked")).toBe(
      "true",
    );
    expect(toast.success).not.toHaveBeenCalled();
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Discard changes")!
      .trigger("click");
    expect(wrapper.get('input[type="number"]').element.value).toBe("0");
    expect(wrapper.get('[role="switch"]').attributes("aria-checked")).toBe(
      "false",
    );
    wrapper.unmount();
  });
});
