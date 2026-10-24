import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed, defineComponent, h, nextTick, ref } from "vue";
import {
  AcmeAPI,
  type AcmeApplicationRecord,
  type AcmeDnsProvider,
} from "../src/lib/api/acme";
import { DDNSAPI, type DDNSTargetDetailPayload } from "../src/lib/api/ddns";
import { useDnsCredentialTransfer } from "../src/composables/useDnsCredentialTransfer";
import { useDDNSResourceLoading } from "../src/views/ddns-management/useDDNSResourceLoading";
import CredentialTransferHint from "../src/components/CredentialTransferHint.vue";
import DDNSProviderCredentialsFields from "../src/views/ddns-management/DDNSProviderCredentialsFields.vue";
import DDNSTargetDialog from "../src/views/ddns-management/DDNSTargetDialog.vue";
import AcmeApplicationDialog from "../src/views/ssl-settings/AcmeApplicationDialog.vue";
import { enShared } from "../../../packages/i18n/src/messages/shared/en";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import type { DnsCredentialTarget } from "../src/lib/dns-credential-bridge";

vi.mock("@admin-shared/utils/toast", () => ({ toast: { success: vi.fn() } }));
const wrappers: ReturnType<typeof mount>[] = [];
const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    missingWarn: false,
    fallbackWarn: false,
    messages: { en: { shared: enShared, admin: enAdmin } },
  });
const fields = ["secret_id", "secret_key"].map((key) => ({
  key,
  label: key,
  type: key === "secret_key" ? ("password" as const) : ("text" as const),
}));
const provider = { name: "tencentcloud", label: "Tencent Cloud", fields };
const acmeProvider: AcmeDnsProvider = {
  dnsType: "dns_tencent",
  label: "Tencent Cloud",
  group: "common",
  credentialSchemes: [
    {
      id: "default",
      label: "Default",
      fields: ["Tencent_SecretId", "Tencent_SecretKey"].map((key) => ({
        key,
        label: key,
        required: true,
      })),
    },
  ],
};
function application(
  id = "acme-1",
  credentials = { Tencent_SecretId: "account", Tencent_SecretKey: "secret" },
): AcmeApplicationRecord {
  return {
    id,
    name: "Certificate",
    dnsType: "dns_tencent",
    credentials,
    domains: ["example.test"],
    primaryDomain: "example.test",
    renewEnabled: false,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
  };
}
function target(
  id = "target-1",
  providerId = "tencentcloud",
): DDNSTargetDetailPayload {
  return {
    id,
    provider: providerId,
    providerLabel: providerId,
    name: "DDNS target",
    domainSummary: "example.test",
    config: { secret_id: "account", secret_key: "secret" },
    enabled: false,
    isPrimary: false,
    sortOrder: 1,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
    updateScope: "ipv4_only",
    lastCheck: { checked_at: null, outcome: null, message: null },
    lastIP: { ipv4: null, ipv6: null, updated_at: null },
    selectionAnchor: { ipv4: null, ipv6: null, updated_at: null },
  };
}
function mockTargets(items: DDNSTargetDetailPayload[]) {
  vi.spyOn(DDNSAPI, "getTargets").mockResolvedValue({
    items,
    total: items.length,
    extraCount: items.length,
    enabledExtraCount: 0,
    primaryTargetId: null,
  });
  vi.spyOn(DDNSAPI, "getTarget").mockImplementation(async (id) =>
    items.find((item) => item.id === id)!,
  );
}
function setup(target: DnsCredentialTarget = "ddns") {
  const providerId = ref(target === "ddns" ? "tencentcloud" : "dns_tencent");
  const credentials = ref<Record<string, string>>({});
  const active = ref(true);
  let transfer!: ReturnType<typeof useDnsCredentialTransfer>;
  const wrapper = mount(
    defineComponent({
      setup() {
        transfer = useDnsCredentialTransfer({
          target,
          providerId,
          targetCredentials: credentials,
          active,
        });
        return () => null;
      },
    }),
    { global: { plugins: [i18n()] } },
  );
  wrappers.push(wrapper);
  return { transfer, credentials, providerId, active, wrapper };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  vi.spyOn(AcmeAPI, "getApplications").mockResolvedValue([application()]);
  vi.spyOn(AcmeAPI, "dnsProviders").mockResolvedValue([acmeProvider]);
});
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount());
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});

describe("credential source discovery and lifecycle", () => {
  it("does not silently switch the explicitly selected source after filling its fields", async () => {
    mockTargets([
      {
        ...target("token-only", "cloudflare"),
        config: { api_token: "shared-token" },
      },
      {
        ...target("with-zone", "cloudflare"),
        config: { api_token: "shared-token", zone_id: "other-zone" },
      },
    ]);
    const { transfer, providerId, credentials } = setup("acme");
    providerId.value = "dns_cf";
    await flushPromises();
    transfer.selectedSourceId.value = "token-only";
    Object.assign(credentials.value, transfer.applySuggestion()!.patch);
    expect(transfer.candidates.value).toHaveLength(1);
    expect(transfer.applySuggestion()).toBeNull();
    transfer.selectedSourceId.value = "with-zone";
    expect(transfer.applySuggestion()?.patch).toEqual({
      CF_Zone_ID: "other-zone",
    });
  });
  it("invalidates compatible-provider suggestions synchronously on provider changes", async () => {
    const { transfer, providerId } = setup();
    await flushPromises();
    expect(transfer.applySuggestion()).not.toBeNull();
    providerId.value = "edgeone";
    expect(transfer.applySuggestion()).toBeNull();
    await flushPromises();
    expect(transfer.applySuggestion()).not.toBeNull();
  });
  it("disambiguates sources whose short ID suffixes collide", async () => {
    vi.mocked(AcmeAPI.getApplications).mockResolvedValue([
      application("first-12345678"),
      application("second-12345678"),
    ]);
    const { transfer } = setup();
    await flushPromises();
    const labels = transfer.candidates.value.map((item) => item.label);
    expect(new Set(labels).size).toBe(2);
  });
  it("finds Tencent credentials even when the most recent application uses another provider", async () => {
    vi.mocked(AcmeAPI.getApplications).mockResolvedValue([
      {
        ...application("latest"),
        dnsType: "dns_cf",
        credentials: { CF_Token: "other" },
      },
      application(),
    ]);
    const { transfer } = setup();
    await flushPromises();
    expect(transfer.applySuggestion()?.patch).toEqual({
      secret_id: "account",
      secret_key: "secret",
    });
  });
  it("finds disabled extra Tencent/EdgeOne targets and retains successes after partial failure", async () => {
    mockTargets([
      target("first", "edgeone"),
      target("second", "edgeone_cname"),
      target("failed"),
      target("unrelated", "dnspod"),
    ]);
    vi.mocked(DDNSAPI.getTarget).mockImplementation(async (id) => {
      if (id === "failed") throw new Error("failed");
      return target(id, id === "first" ? "edgeone" : "edgeone_cname");
    });
    const { transfer } = setup("acme");
    await flushPromises();
    expect(DDNSAPI.getTarget).not.toHaveBeenCalledWith("unrelated");
    expect(transfer.candidates.value).toHaveLength(2);
    expect(transfer.hasSourceError.value).toBe(true);
    expect(transfer.applySuggestion()).toBeNull();
    transfer.selectedSourceId.value = "second";
    expect(transfer.applySuggestion()?.patch).toEqual({
      Tencent_SecretId: "account",
      Tencent_SecretKey: "secret",
    });
  });
  it("requires a source selection, distinguishes duplicate labels, and rechecks conflicts before applying", async () => {
    vi.mocked(AcmeAPI.getApplications).mockResolvedValue([
      application("account-a"),
      application("account-b", {
        Tencent_SecretId: "second",
        Tencent_SecretKey: "second-secret",
      }),
    ]);
    const { transfer, credentials } = setup();
    await flushPromises();
    expect(transfer.candidates.value[0]?.label).not.toBe(
      transfer.candidates.value[1]?.label,
    );
    expect(transfer.applySuggestion()).toBeNull();
    transfer.selectedSourceId.value = "account-b";
    expect(transfer.applySuggestion()?.patch).toEqual({
      secret_id: "second",
      secret_key: "second-secret",
    });
    credentials.value = { secret_id: "account" };
    expect(transfer.applySuggestion()).toBeNull();
    transfer.selectedSourceId.value = "account-a";
    expect(transfer.applySuggestion()?.patch).toEqual({ secret_key: "secret" });
  });
  it("ignores empty and platform-unsupported sources", async () => {
    vi.mocked(AcmeAPI.getApplications).mockResolvedValue([
      {
        ...application(),
        dnsType: "dns_baiducloud",
        credentials: { BAIDU_ACCESS_KEY_ID: "id" },
      },
    ]);
    const { transfer, providerId } = setup();
    providerId.value = "baiducloud";
    await flushPromises();
    expect(transfer.candidates.value).toEqual([]);
    vi.mocked(AcmeAPI.getApplications).mockResolvedValue([
      application("empty", { Tencent_SecretId: "", Tencent_SecretKey: " " }),
    ]);
    providerId.value = "tencentcloud";
    await flushPromises();
    expect(transfer.candidates.value).toEqual([]);
  });
  it("discards stale responses when switching provider and after unmount", async () => {
    const pending = deferred<AcmeApplicationRecord[]>();
    vi.mocked(AcmeAPI.getApplications).mockReturnValueOnce(pending.promise);
    const { transfer, providerId, wrapper } = setup();
    providerId.value = "unknown";
    await nextTick();
    pending.resolve([application()]);
    await flushPromises();
    expect(transfer.candidates.value).toEqual([]);
    providerId.value = "tencentcloud";
    await nextTick();
    wrapper.unmount();
    await flushPromises();
    expect(transfer.candidates.value).toEqual([]);
  });
  it("refreshes on reopening the same provider and rejects responses from a closed dialog", async () => {
    const pending = deferred<AcmeApplicationRecord[]>();
    vi.mocked(AcmeAPI.getApplications).mockReturnValueOnce(pending.promise);
    const { transfer, active } = setup();
    active.value = false;
    await nextTick();
    pending.resolve([application()]);
    await flushPromises();
    expect(transfer.candidates.value).toEqual([]);
    active.value = true;
    await flushPromises();
    expect(AcmeAPI.getApplications).toHaveBeenCalledTimes(2);
    expect(transfer.candidates.value).toHaveLength(1);
  });
  it("recovers from source errors by retrying", async () => {
    vi.mocked(AcmeAPI.getApplications).mockRejectedValueOnce(
      new Error("failed"),
    );
    const { transfer } = setup();
    await flushPromises();
    expect(transfer.hasSourceError.value).toBe(true);
    await transfer.refreshSource();
    expect(transfer.hasSourceError.value).toBe(false);
    expect(transfer.candidates.value).toHaveLength(1);
  });
});

function fillButton(wrapper: ReturnType<typeof mount>) {
  const button = wrapper
    .findAll("button")
    .find((item) => item.text().startsWith("Fill empty fields"));
  if (!button) throw new Error("Missing fill button");
  return button;
}
const dialogStubs = {
  Dialog: { template: "<div><slot /></div>" },
  DialogContent: { template: "<div><slot /></div>" },
  DialogTitle: { template: "<h2><slot /></h2>" },
  DialogDescription: { template: "<p><slot /></p>" },
};

describe("credential fill entry points", () => {
  it("asks the user to reselect when a fill leaves only a different source", async () => {
    mockTargets([
      {
        ...target("token-only", "cloudflare"),
        config: { api_token: "shared-token" },
      },
      {
        ...target("with-zone", "cloudflare"),
        config: { api_token: "shared-token", zone_id: "other-zone" },
      },
    ]);
    const credentials = ref<Record<string, string>>({});
    const wrapper = mount(
      defineComponent({
        setup: () => () =>
          h(CredentialTransferHint, {
            target: "acme",
            providerId: "dns_cf",
            credentials: credentials.value,
            onApply: (patch: Record<string, string>) =>
              Object.assign(credentials.value, patch),
          }),
      }),
      { global: { plugins: [i18n()] } },
    );
    wrappers.push(wrapper);
    await flushPromises();
    wrapper
      .findComponent({ name: "Select" })
      .vm.$emit("update:modelValue", "token-only");
    await nextTick();
    await fillButton(wrapper).trigger("click");
    expect(credentials.value).toEqual({ CF_Token: "shared-token" });
    expect(wrapper.get('[role="combobox"]').text()).toContain(
      "Choose a source",
    );
    expect(fillButton(wrapper).attributes("disabled")).toBeDefined();
    wrapper
      .findComponent({ name: "Select" })
      .vm.$emit("update:modelValue", "with-zone");
    await nextTick();
    await fillButton(wrapper).trigger("click");
    expect(credentials.value).toEqual({
      CF_Token: "shared-token",
      CF_Zone_ID: "other-zone",
    });
  });
  it("waits for the saved DDNS form before allowing a credential fill", async () => {
    const pending = deferred<Record<string, string>>();
    vi.spyOn(DDNSAPI, "getConfig").mockReturnValue(pending.promise);
    const credentials = ref<Record<string, string>>({});
    let loading!: ReturnType<typeof useDDNSResourceLoading>;
    const wrapper = mount(
      defineComponent({
        setup() {
          loading = useDDNSResourceLoading({
            applyStatus: () => {},
            currentProviderDef: computed(() => provider),
            ensurePasswordFieldsVisible: () => {},
            isInitialized: ref(true),
            isPrimaryConfigDirty: ref(false),
            networkInterfaces: ref([]),
            providerConfig: credentials,
            providers: ref([provider]),
            resetFieldEditReady: () => {},
            savedProviderConfig: ref({}),
            selectedProvider: ref("tencentcloud"),
          });
          return () =>
            h(CredentialTransferHint, {
              target: "ddns",
              providerId: "tencentcloud",
              credentials: credentials.value,
              disabled: loading.isLoadingConfig.value,
              onApply: (patch: Record<string, string>) =>
                Object.assign(credentials.value, patch),
            });
        },
      }),
      { global: { plugins: [i18n()] } },
    );
    wrappers.push(wrapper);
    const load = loading.loadConfig();
    await flushPromises();
    expect(fillButton(wrapper).attributes("disabled")).toBeDefined();
    pending.resolve({ secret_id: "account" });
    await load;
    await flushPromises();
    expect(fillButton(wrapper).attributes("disabled")).toBeUndefined();
    await fillButton(wrapper).trigger("click");
    expect(credentials.value.secret_id).toBe("account");
    expect(credentials.value.secret_key).toBe("secret");
  });
  it("fills the DDNS primary form without saving or exposing secrets in the suggestion", async () => {
    const credentials = ref<Record<string, string>>({
      secret_id: "account",
      domain: "preserve.test",
    });
    const enableFieldEditing = vi.fn();
    const save = vi.spyOn(DDNSAPI, "saveConfig").mockResolvedValue();
    const wrapper = mount(
      defineComponent({
        components: { DDNSProviderCredentialsFields },
        setup: () => ({
          credentials,
          provider,
          enableFieldEditing,
          setFieldValue: (key: string, value: string) => {
            credentials.value[key] = value;
          },
        }),
        template: `<DDNSProviderCredentialsFields :provider-config="credentials" :provider-def="provider" :set-field-value="setFieldValue" :enable-field-editing="enableFieldEditing" :field-visibility="{}" :format-domain-field="() => {}" :get-field-autocomplete="() => 'off'" :get-field-description="() => ''" :get-field-dom-id="i => 'field-' + i" :get-field-input-name="i => 'field-' + i" :is-field-edit-ready="() => true" :toggle-field-visibility="() => {}" />`,
      }),
      { global: { plugins: [i18n()] } },
    );
    wrappers.push(wrapper);
    await flushPromises();
    expect(
      wrapper.get('[data-testid="credential-transfer"]').text(),
    ).not.toContain("account");
    await fillButton(wrapper).trigger("click");
    expect(credentials.value).toEqual({
      secret_id: "account",
      secret_key: "secret",
      domain: "preserve.test",
    });
    expect(enableFieldEditing).toHaveBeenCalledWith("secret_key");
    expect(save).not.toHaveBeenCalled();
  });
  it("fills extra target credentials and refreshes when reopened", async () => {
    const state = {
      id: null,
      name: "extra",
      enabled: true,
      provider: "tencentcloud",
      config: {} as Record<string, string>,
    };
    const create = vi.spyOn(DDNSAPI, "createTarget");
    const update = vi.spyOn(DDNSAPI, "updateTarget");
    const wrapper = mount(DDNSTargetDialog, {
      props: {
        open: true,
        state,
        providerDef: provider,
        providers: [provider],
        description: "",
        title: "Target",
        isSaving: false,
        updateProxy: {
          enabled: false,
          url: "",
          username: "",
          passwordConfigured: false,
        },
        formatDomainField: () => {},
        formatOptionLabel: () => "",
        getFieldAutocomplete: () => "off",
        getFieldDescription: () => "",
        isFieldVisible: () => false,
        isIpSourceOptionDisabled: () => false,
        isUpdateScopeOptionDisabled: () => false,
        networkInterfaceLabel: "",
        resolvedNetworkInterfaces: [],
        shouldShowDomainBlock: false,
        shouldShowInterfaceBlock: false,
        shouldShowStaticBlock: false,
        toggleFieldVisibility: () => {},
        updateScope: "ipv4_only",
        ipv4Options: [],
        ipv6Options: [],
      },
      global: {
        plugins: [i18n()],
        stubs: {
          ...dialogStubs,
          DDNSTargetAddressFields: true,
          DDNSTargetBasicFields: true,
        },
      },
    });
    wrappers.push(wrapper);
    await flushPromises();
    await fillButton(wrapper).trigger("click");
    expect(state.config).toEqual({
      secret_id: "account",
      secret_key: "secret",
    });
    expect(wrapper.emitted("confirm")).toBeUndefined();
    expect(create).not.toHaveBeenCalled();
    expect(update).not.toHaveBeenCalled();
    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    await flushPromises();
    expect(AcmeAPI.getApplications).toHaveBeenCalledTimes(2);
  });
  it("fills ACME fields from an extra EdgeOne target without submitting", async () => {
    mockTargets([target("extra", "edgeone")]);
    const create = vi.spyOn(AcmeAPI, "createApplication");
    const update = vi.spyOn(AcmeAPI, "updateApplication");
    const wrapper = mount(AcmeApplicationDialog, {
      props: {
        open: true,
        mode: "edit",
        initialValue: application("edit", {
          Tencent_SecretId: "",
          Tencent_SecretKey: "",
        }),
        dnsProviders: [acmeProvider],
      },
      global: {
        plugins: [i18n()],
        stubs: { ...dialogStubs, AcmeFileOutputSettings: true },
      },
    });
    wrappers.push(wrapper);
    await flushPromises();
    await fillButton(wrapper).trigger("click");
    const values = wrapper
      .findAll('[data-acme-dialog="credentials"] input')
      .map((input) => (input.element as HTMLInputElement).value);
    expect(values).toContain("account");
    expect(values).toContain("secret");
    expect(wrapper.emitted("submit")).toBeUndefined();
    expect(create).not.toHaveBeenCalled();
    expect(update).not.toHaveBeenCalled();
  });
  it("requires choosing a source in the shared UI and blocks conflicting fills", async () => {
    vi.mocked(AcmeAPI.getApplications).mockResolvedValue([
      application("first"),
      application("second", {
        Tencent_SecretId: "different",
        Tencent_SecretKey: "second-secret",
      }),
    ]);
    const wrapper = mount(CredentialTransferHint, {
      props: {
        target: "ddns",
        providerId: "tencentcloud",
        credentials: { secret_id: "account" },
      },
      global: { plugins: [i18n()] },
    });
    wrappers.push(wrapper);
    await flushPromises();
    expect(fillButton(wrapper).attributes("disabled")).toBeDefined();
    const select = wrapper.findComponent({ name: "Select" });
    select.vm.$emit("update:modelValue", "second");
    await nextTick();
    expect(wrapper.text()).toContain("Existing fields are incompatible");
    expect(fillButton(wrapper).attributes("disabled")).toBeDefined();
    select.vm.$emit("update:modelValue", "first");
    await nextTick();
    await fillButton(wrapper).trigger("click");
    expect(wrapper.emitted("apply")).toEqual([[{ secret_key: "secret" }]]);
  });
});
