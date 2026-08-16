import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import type { StreamServiceCatalog } from "../src/lib/api/config";
import type { StreamMapping } from "../src/types";
import StreamServiceProfileDialog from "../src/views/stream-mappings/StreamServiceProfileDialog.vue";

const mapping = {
  listen_port: 6788,
  protocol: "tcp",
  service_profile: {
    service_id: "easytier",
    source: "probe",
    strict_capable: false,
  },
  target: "192.168.31.98:11010",
} as StreamMapping;

const catalog: StreamServiceCatalog = {
  classifier_version: "stream-signatures-v4",
  items: [
    {
      active_probe_supported: true,
      display_name: "EasyTier",
      service_family: "vpn",
      service_id: "easytier",
      strict_capable: false,
      transports: ["tcp"],
    },
    {
      active_probe_supported: true,
      display_name: "WebDAV",
      service_family: "file_service",
      service_id: "webdav",
      strict_capable: true,
      transports: ["tcp"],
    },
    {
      active_probe_supported: true,
      display_name: "ONVIF",
      service_family: "video",
      service_id: "onvif",
      strict_capable: true,
      transports: ["udp"],
    },
  ],
};

const passthrough = { template: "<div><slot /></div>" };

describe("stream service profile dialog", () => {
  it("offers identification-only TCP services and explains their safe behavior", async () => {
    const wrapper = mount(StreamServiceProfileDialog, {
      props: {
        catalog,
        initialServiceId: "easytier",
        loading: false,
        mapping,
        open: false,
      },
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: {
              en: {
                admin: {
                  streamMappings: {
                    cancel: "Cancel",
                    confirmService: "Confirm and enable validation",
                    confirmServiceIdentificationOnly: "Confirm service type",
                    selectServiceDescription: "Select a service",
                    selectServiceIdentificationOnlyWarning:
                      "Strict validation remains off and the mapping stays enabled.",
                    selectServicePlaceholder: "Select",
                    selectServiceTitle: "Specify service type",
                    selectServiceWarning:
                      "A wrong strict choice rejects connections.",
                    serviceIdentificationOnly: "Identification only",
                    serviceProfile: "Service identification",
                  },
                },
              },
            },
          }),
        ],
        stubs: {
          Button: {
            props: ["disabled"],
            template: '<button :disabled="disabled"><slot /></button>',
          },
          Dialog: passthrough,
          DialogContent: passthrough,
          DialogDescription: passthrough,
          DialogFooter: passthrough,
          DialogHeader: passthrough,
          DialogTitle: passthrough,
          Label: passthrough,
        },
      },
    });

    await wrapper.setProps({ open: true });

    const select = wrapper.get("select");
    expect(select.element.value).toBe("easytier");
    expect(
      select.findAll("option").map((option) => option.attributes("value")),
    ).toEqual(["", "easytier", "webdav"]);
    expect(wrapper.text()).toContain("EasyTier · vpn · Identification only");
    expect(wrapper.text()).toContain(
      "Strict validation remains off and the mapping stays enabled.",
    );
    expect(wrapper.text()).toContain("Confirm service type");
    expect(wrapper.text()).not.toContain(
      "A wrong strict choice rejects connections.",
    );

    await select.setValue("webdav");
    expect(wrapper.text()).toContain("Confirm and enable validation");
    expect(wrapper.text()).toContain(
      "A wrong strict choice rejects connections.",
    );
    wrapper.unmount();
  });
});
