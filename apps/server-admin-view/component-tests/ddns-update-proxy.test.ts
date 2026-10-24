import { defineComponent, ref } from "vue";
import { mount, flushPromises } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import DDNSUpdateProxyFields from "../src/views/ddns-management/DDNSUpdateProxyFields.vue";
import DDNSUpdateProxyModeField from "../src/views/ddns-management/DDNSUpdateProxyModeField.vue";
import {
  createUpdateProxyDraft,
  normalizeUpdateProxy,
  normalizeUpdateProxyMode,
  updateProxyValidationKey,
} from "../src/views/ddns-management/ddns-proxy-model";
import {
  extractCommonTargetConfig,
  isProviderConfigEqual,
} from "../src/views/ddns-management/model";
import { useDDNSSettingsDialogs } from "../src/views/ddns-management/useDDNSSettingsDialogs";
import { DDNSAPI } from "../src/lib/api/ddns";
import { toast } from "@admin-shared/utils/toast";

const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: { en: { admin: enAdmin } },
  });
afterEach(() => vi.restoreAllMocks());

describe("DDNS update proxy", () => {
  it("validates proxy addresses and preserves target overrides in common config and dirty checks", () => {
    for (const url of [
      "http://localhost:7890",
      "https://proxy.example:443",
      "socks5://[::1]:1080",
      "socks5h://localhost:1080",
    ]) {
      expect(
        updateProxyValidationKey({
          ...createUpdateProxyDraft(),
          url,
          enabled: true,
        }),
      ).toBeNull();
    }
    for (const url of [
      "ftp://localhost:80",
      "socks5://localhost",
      "http://u:p@localhost:80",
      "http://localhost:0",
      "https://localhost/path",
    ]) {
      expect(
        updateProxyValidationKey({ ...createUpdateProxyDraft(), url }),
      ).toBe("admin.ddns.updateProxyInvalid");
    }
    expect(
      updateProxyValidationKey({ ...createUpdateProxyDraft(), enabled: true }),
    ).toBe("admin.ddns.updateProxyMissing");
    expect(normalizeUpdateProxyMode(undefined)).toBe("inherit");
    expect(
      extractCommonTargetConfig({ update_proxy_mode: "proxy" })
        .update_proxy_mode,
    ).toBe("proxy");
    expect(isProviderConfigEqual({}, { update_proxy_mode: "inherit" })).toBe(
      true,
    );
    expect(isProviderConfigEqual({}, { update_proxy_mode: "direct" })).toBe(
      false,
    );
  });

  it("never fills saved passwords and supports explicit clearing and replacement", async () => {
    const host = defineComponent({
      components: { DDNSUpdateProxyFields },
      setup: () => ({
        draft: ref(
          createUpdateProxyDraft({
            passwordConfigured: true,
            url: "http://localhost:7890",
            username: "u",
          }),
        ),
      }),
      template: '<DDNSUpdateProxyFields v-model="draft" :disabled="false" />',
    });
    const wrapper = mount(host, { global: { plugins: [i18n()] } });
    const password = wrapper.get<HTMLInputElement>(
      "#ddns-update-proxy-password",
    );
    expect(password.element.value).toBe("");
    expect(password.attributes("type")).toBe("password");
    await wrapper.get("button[aria-pressed]").trigger("click");
    expect(wrapper.vm.draft.clearPassword).toBe(true);
    await password.setValue("replacement");
    expect(wrapper.vm.draft.clearPassword).toBe(false);
    expect(wrapper.vm.draft.password).toBe("replacement");
    expect(wrapper.text()).toContain(
      "Public IP detection always connects directly",
    );
    wrapper.unmount();
  });

  it("shows current global routing and missing proxy feedback", async () => {
    const wrapper = mount(DDNSUpdateProxyModeField, {
      props: { id: "mode", defaultEnabled: false, configured: false },
      global: { plugins: [i18n()] },
    });
    expect(wrapper.text()).toContain("Follow global (Direct)");
    await wrapper.setProps({ defaultEnabled: true, configured: true });
    expect(wrapper.text()).toContain("Follow global (Use proxy)");
    await wrapper.setProps({ modelValue: "proxy", configured: false });
    expect(wrapper.find('[role="alert"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it("keeps settings drafts separate, preserves failures, prevents duplicate saves, and tests IP sources directly", async () => {
    vi.spyOn(toast, "success").mockImplementation(() => undefined);
    vi.spyOn(toast, "error").mockImplementation(() => undefined);
    const savedProxy = ref(
      normalizeUpdateProxy({
        url: "http://localhost:7890",
        username: "u",
        passwordConfigured: true,
      }),
    );
    const sources = { ipv4: ["https://ip.example"], ipv6: [] };
    let controller!: ReturnType<typeof useDDNSSettingsDialogs>;
    const host = defineComponent({
      setup() {
        controller = useDDNSSettingsDialogs({
          updateProxy: savedProxy,
          defaultPublicCheckSources: ref(sources),
          publicCheckSources: ref(sources),
          httpTransport: ref("node"),
          publicDnsProvider: ref("none"),
          providerConfig: ref({}),
          updateIntervalMinutes: ref(10),
        });
        return () => null;
      },
    });
    const wrapper = mount(host, { global: { plugins: [i18n()] } });
    controller.openPublicCheckDialog();
    controller.updateProxyDraft.value.enabled = true;
    controller.updateProxyDraft.value.password = "new-secret";
    controller.showPublicCheckDialog.value = false;
    controller.openPublicCheckDialog();
    expect(controller.updateProxyDraft.value.password).toBe("");
    expect(controller.updateProxyDraft.value.enabled).toBe(false);
    const save = vi
      .spyOn(DDNSAPI, "saveSettings")
      .mockRejectedValueOnce(new Error("offline"));
    controller.updateProxyDraft.value.enabled = true;
    await controller.savePublicCheckSources(sources, "node", "none");
    expect(controller.showPublicCheckDialog.value).toBe(true);
    expect(controller.updateProxyDraft.value.enabled).toBe(true);
    expect(savedProxy.value.enabled).toBe(false);
    const result = {
      ...sources,
      updateProxy: { ...savedProxy.value, enabled: true },
      httpTransport: "node" as const,
      publicDnsProvider: "none" as const,
      publicCheckSources: sources,
      defaultPublicCheckSources: sources,
      updateIntervalMinutes: 10,
    };
    let resolve!: (value: typeof result) => void;
    save.mockImplementation(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const pending = controller.savePublicCheckSources(sources, "node", "none");
    await controller.savePublicCheckSources(sources, "node", "none");
    expect(save).toHaveBeenCalledTimes(2);
    resolve(result);
    await pending;
    expect(savedProxy.value.enabled).toBe(true);
    expect(controller.showPublicCheckDialog.value).toBe(false);
    expect(save.mock.calls[1]![0].updateProxy).not.toHaveProperty(
      "passwordConfigured",
    );
    const test = vi
      .spyOn(DDNSAPI, "testPublicCheckSources")
      .mockResolvedValue({ results: [] });
    await controller.testPublicCheckSources(sources, "node", "none");
    expect(test.mock.calls[0]![1]).not.toHaveProperty("updateProxy");
    await flushPromises();
    wrapper.unmount();
  });
});
