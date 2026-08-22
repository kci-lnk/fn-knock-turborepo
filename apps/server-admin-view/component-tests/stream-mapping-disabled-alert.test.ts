import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";

import StreamMappingDisabledAlert from "../src/views/stream-mappings/StreamMappingDisabledAlert.vue";

const createTestI18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        admin: {
          streamMappings: {
            disabledDescription: "Disabled by the administrator.",
            disabledTitle: "Protocol mappings are disabled",
            runtimeDisabledTitle: "Protocol mappings failed to start",
            runtimeIssueDetails: "Startup error details",
            runtimeIssueFallback: "The gateway rejected the configuration.",
            runtimeIssueLocalLoop: "{protocol} port {port} loops to {target}.",
            runtimeIssuePortInUse: "{protocol} port {port} is already in use.",
            runtimeIssueRecovery: "Fix the rule and re-enable the feature.",
          },
        },
      },
    },
  });

describe("stream mapping disabled alert", () => {
  it("shows an occupied port and the original gateway error", () => {
    const wrapper = mount(StreamMappingDisabledAlert, {
      props: {
        runtimeIssue: {
          code: "listen_port_in_use",
          listen_port: 9000,
          message: "listen tcp :9000: bind: address already in use",
          protocol: "tcp",
          target: "127.0.0.1:9001",
        },
      },
      global: { plugins: [createTestI18n()] },
    });

    expect(wrapper.text()).toContain("Protocol mappings failed to start");
    expect(wrapper.text()).toContain("TCP port 9000 is already in use");
    expect(wrapper.text()).toContain(
      "listen tcp :9000: bind: address already in use",
    );
  });

  it("keeps the ordinary disabled state free of stale runtime details", () => {
    const wrapper = mount(StreamMappingDisabledAlert, {
      global: { plugins: [createTestI18n()] },
    });

    expect(wrapper.text()).toContain("Protocol mappings are disabled");
    expect(wrapper.text()).toContain("Disabled by the administrator");
    expect(wrapper.text()).not.toContain("Startup error details");
  });
});
