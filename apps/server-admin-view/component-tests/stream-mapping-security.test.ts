import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConfigAPI } from "../src/lib/api/config";
import { useConfigStore } from "../src/store/config";
import type { AppConfig, StreamMapping } from "../src/types";
import { useStreamMappingSecurity } from "../src/views/stream-mappings/useStreamMappingSecurity";

const toastMocks = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
}));

vi.mock("@admin-shared/utils/toast", () => ({ toast: toastMocks }));

const mapping: StreamMapping = {
  comment: "WebDAV",
  listen_port: 6006,
  probe_status: "unknown",
  protocol: "tcp",
  service_profile: {
    classifier_version: "stream-signatures-v3",
    device_role: "web_service",
    evidence_codes: ["http_status_line"],
    metadata: {
      auth_probe_status: "401",
      auth_scheme: "basic",
      http_status: "200",
    },
    observed_at: "2026-08-16T00:00:00Z",
    role_confidence: "strong",
    service_confidence: "strong",
    service_family: "web",
    service_id: "http1",
    source: "probe",
    strict_capable: true,
    target_fingerprint: "fingerprint",
  },
  target: "127.0.0.1:5005",
  use_auth: true,
};

describe("stream mapping service detection", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    toastMocks.error.mockReset();
    toastMocks.success.mockReset();
    toastMocks.warning.mockReset();
  });

  it("opens manual service confirmation when upstream auth hides WebDAV", async () => {
    const configStore = useConfigStore();
    configStore.config = {
      host_mapping_grouped_view: false,
      host_mapping_groups: [],
      host_mappings: [],
      stream_mappings: [mapping],
    } as AppConfig;
    const loadConfig = vi.spyOn(configStore, "loadConfig");
    vi.spyOn(ConfigAPI, "probeStreamMapping").mockResolvedValue({
      profile: mapping.service_profile,
      message:
        "HTTP authentication challenge hides the application service; manual confirmation is required",
      status: "unknown",
    });
    vi.spyOn(ConfigAPI, "getStreamMappings").mockResolvedValue([mapping]);
    vi.spyOn(ConfigAPI, "getStreamServiceCatalog").mockResolvedValue({
      classifier_version: "stream-signatures-v3",
      items: [
        {
          active_probe_supported: true,
          display_name: "WebDAV",
          service_family: "file_service",
          service_id: "webdav",
          strict_capable: true,
          transports: ["tcp"],
        },
      ],
    });

    let model!: ReturnType<typeof useStreamMappingSecurity>;
    const Harness = defineComponent({
      setup() {
        model = useStreamMappingSecurity();
        return () => null;
      },
    });
    const wrapper = mount(Harness, {
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: {
              en: {
                admin: {
                  streamMappings: {
                    probeAuthenticatedHttp: "HTTP authentication hides the service",
                    probeAuthenticatedHttpDescription:
                      "Confirm WebDAV manually",
                  },
                },
              },
            },
          }),
        ],
      },
    });

    await model.probeMapping(mapping);

    expect(loadConfig).not.toHaveBeenCalled();
    expect(toastMocks.warning).toHaveBeenCalledOnce();
    expect(model.isServiceProfileOpen.value).toBe(true);
    expect(model.serviceProfileInitialServiceId.value).toBe("");
    expect(model.serviceProfileMapping.value?.target).toBe("127.0.0.1:5005");
    expect(model.serviceCatalog.value?.items[0]?.service_id).toBe("webdav");
    wrapper.unmount();
  });

  it("keeps an unknown but reachable target enabled without strict validation", async () => {
    const unknownMapping: StreamMapping = {
      ...mapping,
      disabled: true,
      service_profile: undefined,
      validation_mode: "strict",
    };
    const configStore = useConfigStore();
    configStore.config = {
      host_mapping_grouped_view: false,
      host_mapping_groups: [],
      host_mappings: [],
      stream_mappings: [unknownMapping],
    } as AppConfig;
    vi.spyOn(ConfigAPI, "probeStreamMapping").mockResolvedValue({
      message: "target responded but no strong service signature matched",
      status: "unknown",
    });
    vi.spyOn(ConfigAPI, "getStreamMappings").mockResolvedValue([
      {
        ...unknownMapping,
        disabled: false,
        probe_status: "unknown",
        validation_mode: "off",
      },
    ]);

    let model!: ReturnType<typeof useStreamMappingSecurity>;
    const Harness = defineComponent({
      setup() {
        model = useStreamMappingSecurity();
        return () => null;
      },
    });
    const wrapper = mount(Harness, {
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: {
              en: {
                admin: {
                  streamMappings: {
                    probeUnverified: "Target reachable without strict validation",
                  },
                },
              },
            },
          }),
        ],
      },
    });

    await model.probeMapping(unknownMapping);

    expect(configStore.config?.stream_mappings?.[0]?.disabled).toBe(false);
    expect(configStore.config?.stream_mappings?.[0]?.validation_mode).toBe("off");
    expect(toastMocks.warning).toHaveBeenCalledWith(
      "Target reachable without strict validation",
      {
        description: "target responded but no strong service signature matched",
      },
    );
    expect(toastMocks.error).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("reports an identification-only EasyTier probe without enabling strict validation", async () => {
    const easyTierMapping: StreamMapping = {
      ...mapping,
      disabled: false,
      service_profile: undefined,
      validation_mode: "off",
    };
    const easyTierProfile = {
      ...mapping.service_profile!,
      device_role: "vpn",
      service_confidence: "strong",
      service_family: "vpn",
      service_id: "easytier",
      strict_capable: false,
    };
    const configStore = useConfigStore();
    configStore.config = {
      host_mapping_grouped_view: false,
      host_mapping_groups: [],
      host_mappings: [],
      stream_mappings: [easyTierMapping],
    } as AppConfig;
    vi.spyOn(ConfigAPI, "probeStreamMapping").mockResolvedValue({
      message: "service identified; strict validation is not enabled",
      profile: easyTierProfile,
      status: "unknown",
    });
    vi.spyOn(ConfigAPI, "getStreamMappings").mockResolvedValue([
      {
        ...easyTierMapping,
        disabled: false,
        probe_status: "unknown",
        service_profile: easyTierProfile,
        validation_mode: "off",
      },
    ]);

    let model!: ReturnType<typeof useStreamMappingSecurity>;
    const Harness = defineComponent({
      setup() {
        model = useStreamMappingSecurity();
        return () => null;
      },
    });
    const wrapper = mount(Harness, {
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: {
              en: {
                admin: {
                  streamMappings: {
                    probeIdentifiedWithoutStrict:
                      "Target identified without strict validation",
                    probeIdentifiedWithoutStrictDescription:
                      "Identified as {service}; mapping enabled.",
                  },
                },
              },
            },
          }),
        ],
      },
    });

    await model.probeMapping(easyTierMapping);

    expect(configStore.config?.stream_mappings?.[0]?.disabled).toBe(false);
    expect(configStore.config?.stream_mappings?.[0]?.validation_mode).toBe("off");
    expect(configStore.config?.stream_mappings?.[0]?.service_profile?.service_id).toBe(
      "easytier",
    );
    expect(toastMocks.warning).toHaveBeenCalledWith(
      "Target identified without strict validation",
      { description: "Identified as easytier; mapping enabled." },
    );
    expect(toastMocks.error).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("clears a manually specified service and refreshes the enabled mapping", async () => {
    const manualMapping: StreamMapping = {
      ...mapping,
      probe_status: "manual",
      service_profile: {
        ...mapping.service_profile!,
        service_id: "webdav_tls",
        source: "manual",
      },
      validation_mode: "strict",
    };
    const configStore = useConfigStore();
    configStore.config = {
      host_mapping_grouped_view: false,
      host_mapping_groups: [],
      host_mappings: [],
      stream_mappings: [manualMapping],
    } as AppConfig;
    const clear = vi
      .spyOn(ConfigAPI, "clearStreamServiceProfile")
      .mockResolvedValue();
    vi.spyOn(ConfigAPI, "getStreamMappings").mockResolvedValue([
      {
        ...manualMapping,
        disabled: false,
        probe_status: "stale",
        service_profile: undefined,
        validation_mode: "off",
      },
    ]);

    let model!: ReturnType<typeof useStreamMappingSecurity>;
    const Harness = defineComponent({
      setup() {
        model = useStreamMappingSecurity();
        return () => null;
      },
    });
    const wrapper = mount(Harness, {
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: {
              en: {
                admin: {
                  streamMappings: {
                    serviceClearFailed: "Clear failed",
                    serviceCleared: "Service cleared",
                  },
                },
                common: { tryLater: "Try later" },
              },
            },
          }),
        ],
      },
    });
    model.serviceProfileMapping.value = manualMapping;
    model.isServiceProfileOpen.value = true;

    await model.clearServiceProfile();

    expect(clear).toHaveBeenCalledWith(manualMapping);
    expect(configStore.config?.stream_mappings?.[0]?.disabled).toBe(false);
    expect(configStore.config?.stream_mappings?.[0]?.service_profile).toBeUndefined();
    expect(model.isServiceProfileOpen.value).toBe(false);
    expect(toastMocks.success).toHaveBeenCalledWith("Service cleared");
    wrapper.unmount();
  });
});
