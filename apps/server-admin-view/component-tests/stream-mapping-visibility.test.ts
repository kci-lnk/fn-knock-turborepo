import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "vue-i18n";
import { nextTick } from "vue";
import MappingVisibilityFields from "@/components/MappingVisibilityFields.vue";
import StreamProtocolMultiSelect from "@/components/StreamProtocolMultiSelect.vue";
import { ConfigAPI } from "@/lib/api/config";
import type { GatewayVisibilityDetails, StreamMapping } from "@/types";
import StreamMappingEditorDialog from "@/views/stream-mappings/StreamMappingEditorDialog.vue";

const gatewayVisibility = (enabled: boolean): GatewayVisibilityDetails => ({
  config: { enabled, selections: [], custom_cidrs: [] },
  summary: {
    enabled,
    cidr_count: 0,
    custom_cidr_count: 0,
    range_count: 0,
    selection_count: 0,
    updated_at: null,
  },
});

const mountEditor = (mapping: StreamMapping | null) =>
  mount(StreamMappingEditorDialog, {
    attachTo: document.body,
    props: {
      existingMappings: mapping ? [mapping] : [],
      mapping,
      open: true,
      saving: false,
    },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          missingWarn: false,
          fallbackWarn: false,
          messages: { en: {} },
        }),
      ],
      stubs: { CidrRegionSelector: true },
    },
  });

const click = async (selector: string) => {
  const element = document.querySelector<HTMLButtonElement>(selector);
  expect(element).toBeTruthy();
  element?.click();
  await flushPromises();
};

const setInput = (id: string, value: string) => {
  const input = document.querySelector<HTMLInputElement>(id);
  if (!input) throw new Error(`Missing dialog input ${id}`);
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
};

afterEach(() => {
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

describe("stream mapping visibility editor", () => {
  it("blocks invalid custom input and saves the same initial policy to both selected protocols", async () => {
    vi.spyOn(ConfigAPI, "getGatewayVisibility").mockResolvedValue(
      gatewayVisibility(true),
    );
    const wrapper = mountEditor(null);
    try {
      await flushPromises();
      setInput("#stream-listen-port", "8053");
      setInput("#stream-target", "127.0.0.1:53");
      wrapper
        .findComponent(StreamProtocolMultiSelect)
        .vm.$emit("update:modelValue", ["udp", "tcp"]);
      await click('[data-slot="collapsible-trigger"]');
      expect(document.body.textContent).toContain(
        "admin.streamMappings.visibilityMultiProtocol",
      );

      const fields = wrapper.getComponent(MappingVisibilityFields);
      fields.vm.$emit("update:mode", "custom");
      fields.vm.$emit("update:customCidrsText", "invalid-network");
      await click('[data-slot="dialog-footer"] button:last-child');
      expect(wrapper.emitted("save")).toBeUndefined();
      expect(document.body.textContent).toContain(
        "admin.subdomainProxy.visibilityInvalidCidrs",
      );

      fields.vm.$emit(
        "update:customCidrsText",
        "198.51.100.0/24\n2001:db8::/32",
      );
      await click('[data-slot="dialog-footer"] button:last-child');
      expect(wrapper.emitted("save")).toEqual([
        [
          {
            editingKey: null,
            mappings: ["tcp", "udp"].map((protocol) => ({
              protocol,
              listen_port: 8053,
              target: "127.0.0.1:53",
              comment: "",
              use_auth: true,
              visibility: {
                mode: "custom",
                selections: [],
                custom_cidrs: ["198.51.100.0/24", "2001:db8::/32"],
              },
            })),
          },
        ],
      ]);
    } finally {
      wrapper.unmount();
    }
  });

  it("keeps the saved policy when status fails to load and when the gateway switch is off", async () => {
    const lookup = vi
      .spyOn(ConfigAPI, "getGatewayVisibility")
      .mockRejectedValueOnce(new Error("gateway status unavailable"))
      .mockResolvedValueOnce(gatewayVisibility(false));
    const mapping: StreamMapping = {
      protocol: "udp",
      listen_port: 8053,
      target: "127.0.0.1:53",
      use_auth: true,
      visibility: {
        mode: "custom",
        selections: [],
        custom_cidrs: ["198.51.100.0/24"],
      },
    };
    const wrapper = mountEditor(mapping);
    try {
      await flushPromises();
      await click('[data-slot="collapsible-trigger"]');
      expect(document.querySelector('[role="alert"]')?.textContent).toContain(
        "gateway status unavailable",
      );
      expect(
        document.querySelector('[data-slot="collapsible-trigger"]')
          ?.textContent,
      ).toContain("admin.subdomainProxy.visibilityCustom");
      setInput("#stream-listen-port", "8054");
      await nextTick();
      await click('[data-slot="dialog-footer"] button:last-child');

      await click('[data-slot="collapsible-content"] button');
      expect(lookup).toHaveBeenCalledTimes(2);
      expect(document.body.textContent).toContain(
        "admin.streamMappings.visibilityGatewayOff",
      );
      expect(wrapper.findComponent(MappingVisibilityFields).exists()).toBe(
        false,
      );
      await click('[data-slot="dialog-footer"] button:last-child');
      const submission = {
        editingKey: "udp:8053",
        mappings: [{ ...mapping, comment: "", listen_port: 8054 }],
      };
      expect(wrapper.emitted("save")).toEqual([[submission], [submission]]);
    } finally {
      wrapper.unmount();
    }
  });
});
