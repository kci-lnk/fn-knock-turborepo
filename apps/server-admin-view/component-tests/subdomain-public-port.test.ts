import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it, vi } from "vitest";
import SubdomainModeConfigCard from "../src/views/subdomain-proxy/SubdomainModeConfigCard.vue";

const mountCard = (overrides = {}) =>
  mount(SubdomainModeConfigCard, {
    props: {
      activeEdgeClientIpProvider: null,
      authServiceMapping: null,
      authServicePublicPort: 7999,
      configured: false,
      edgeClientIpEnabled: false,
      edgeClientIpProviderOptions: [],
      formatAuthServiceHost: (host: string) => host,
      isEdgeClientIpModeEditable: false,
      isModeDirty: false,
      isModeValid: true,
      isSavingMappings: false,
      isSavingMode: false,
      omitPublicPortConfiguration: false,
      ready: true,
      removeAuthService: vi.fn(),
      resetModeForm: vi.fn(),
      rootDomain: "example.com",
      rootDomainValidationMessage: "",
      saveMode: vi.fn(),
      savedEdgeClientIpProviderLabel: "",
      savedRootDomain: "example.com",
      selectEdgeClientIpProvider: vi.fn(),
      ...overrides,
    },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          missingWarn: false,
          fallbackWarn: false,
        }),
      ],
      stubs: { ConfigCollapsibleCard: { template: "<div><slot /></div>" } },
    },
  });

describe("subdomain public port configuration", () => {
  it("allows FRP HTTPS port editing despite stale edge settings", async () => {
    const wrapper = mountCard({ edgeClientIpEnabled: true });
    await wrapper.get("#auth-service-public-port").setValue("443");
    expect(wrapper.emitted("update:authServicePublicPort")).toEqual([[443]]);
    wrapper.unmount();
  });

  it.each([
    { omitPublicPortConfiguration: true },
    { isEdgeClientIpModeEditable: true, edgeClientIpEnabled: true },
  ])("hides the port for ingress with a managed public port: %o", (props) => {
    const wrapper = mountCard(props);
    expect(wrapper.find("#auth-service-public-port").exists()).toBe(false);
    wrapper.unmount();
  });
});
