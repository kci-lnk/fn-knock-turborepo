import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defineComponent, ref } from "vue";
import AcmeFileOutputSettings from "../src/views/ssl-settings/AcmeFileOutputSettings.vue";
import { ConfigAPI } from "../src/lib/api/config";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

vi.mock("../src/store/config", () => ({
  useConfigStore: () => ({ isDockerDeployment: true }),
}));
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});
function setup(enabledInitially = true) {
  const wrapper = mount(
    defineComponent({
      components: { AcmeFileOutputSettings },
      setup: () => ({
        enabled: ref(enabledInitially),
        directory: ref("/mnt/certs"),
      }),
      template:
        '<AcmeFileOutputSettings v-model:enabled="enabled" v-model:directory="directory" domain="*.Example.TEST" />',
    }),
    {
      attachTo: document.body,
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: { en: { admin: enAdmin, common: { cancel: "Cancel" } } },
          }),
        ],
      },
    },
  );
  wrappers.push(wrapper);
  return wrapper;
}
function clickButton(text: string) {
  const button = [...document.querySelectorAll("button")].find(
    (b) => b.textContent?.trim() === text,
  );
  if (!button) throw new Error(`Missing button: ${text}`);
  button.click();
}
function mockBrowser() {
  vi.spyOn(ConfigAPI, "browseHostMappingStaticPath").mockResolvedValue({
    breadcrumbs: [],
    current_path: "/mnt/new",
    current_selectable: true,
    entries: [],
    error_code: null,
    next_cursor: null,
    parent_path: "/mnt",
    platform: "posix",
    previous_cursor: null,
    selected_path: null,
    target_type: "directory",
  });
  vi.spyOn(ConfigAPI, "probeHostMappingStaticPath").mockResolvedValue({
    actual_type: "directory",
    error_code: null,
    exists: true,
    normalized_path: "/mnt/new",
    readable: true,
    target_type: "directory",
  });
}

describe("ACME certificate file output settings", () => {
  it("retains the directory when toggled and previews normalized wildcard filenames", async () => {
    const wrapper = setup(false);
    expect(wrapper.find("input").exists()).toBe(false);
    await wrapper.get('[role="switch"]').trigger("click");
    expect(wrapper.get('[data-testid="acme-output-paths"]').text()).toContain(
      "/mnt/certs/wildcard.example.test.cert.pem",
    );
    await wrapper.get("input").setValue("/mnt/证书/new");
    expect(wrapper.text()).toContain(
      "/mnt/证书/new/wildcard.example.test.key.pem",
    );
    await wrapper.get('[role="switch"]').trigger("click");
    await wrapper.get('[role="switch"]').trigger("click");
    expect((wrapper.get("input").element as HTMLInputElement).value).toBe(
      "/mnt/证书/new",
    );
    expect(wrapper.text()).toContain(enAdmin.acmeFileOutput.overwriteHelp);
  });
  it("reuses ordinary directory browsing and applies a confirmed selection", async () => {
    mockBrowser();
    const wrapper = setup();
    clickButton(enAdmin.acmeFileOutput.browse);
    await flushPromises();
    expect(ConfigAPI.browseHostMappingStaticPath).toHaveBeenCalledWith(
      "directory",
      "/mnt/certs",
      null,
    );
    clickButton(enAdmin.acmeFileOutput.selectDirectory);
    await flushPromises();
    expect(ConfigAPI.probeHostMappingStaticPath).toHaveBeenCalledWith(
      "directory",
      "/mnt/new",
    );
    expect((wrapper.get("input").element as HTMLInputElement).value).toBe(
      "/mnt/new",
    );
  });
  it("canceling the browser preserves the staged directory", async () => {
    mockBrowser();
    const wrapper = setup();
    clickButton(enAdmin.acmeFileOutput.browse);
    await flushPromises();
    clickButton("Cancel");
    await flushPromises();
    expect((wrapper.get("input").element as HTMLInputElement).value).toBe(
      "/mnt/certs",
    );
    expect(ConfigAPI.probeHostMappingStaticPath).not.toHaveBeenCalled();
  });
});

// Exercise the row status and retry action without invoking real ACME services.
describe("ACME file output status", () => {
  it("keeps issuance success visible alongside a save error and retries the selected application", async () => {
    const { default: ApplicationsTable } =
      await import("../src/views/ssl-settings/AcmeCertificateApplicationsTable.vue");
    const application = {
      id: "a",
      domains: ["example.test"],
      primaryDomain: "example.test",
      dnsType: "dns_cf",
      providerLabel: "Cloudflare",
      renewEnabled: true,
      certificate: { exists: true },
      library: { linked: true },
      latestJob: { status: "succeeded" },
      fileOutput: { enabled: true, directory: "/mnt/certs" },
      fileOutputStatus: {
        status: "error",
        error: "Permission denied",
        lastSuccessAt: "2026-09-01T00:00:00Z",
      },
    };
    const syncFileOutput = vi.fn();
    const controller = {
      applications: ref([application]),
      isOverviewLoading: ref(false),
      isTableLocked: ref(false),
      isActionBlocked: () => false,
      isConfigurationEditBlocked: () => false,
      isDeleteApplicationBlocked: () => false,
      isSecondaryActionDisabled: () => false,
      certificateBadgeVariant: () => "outline",
      certificateStatusLabel: () => "Issued",
      libraryBadgeVariant: () => "outline",
      libraryStatusLabel: () => "Linked",
      jobBadgeVariant: () => "outline",
      latestJobLabel: () => "Issuance succeeded",
      formatCertificateRange: () => "Valid",
      primaryActionLabel: () => "Renew",
      deleteApplicationDescription: () => "Delete application",
      formatFileOutputTime: () => "2026/09/01",
      syncFileOutput,
      t: (key: string) =>
        (key
          .split(".")
          .reduce<unknown>(
            (value, part) => (value as Record<string, unknown>)?.[part],
            { admin: enAdmin },
          ) as string) || key,
    };
    const wrapper = mount(ApplicationsTable, {
      props: {
        controller:
          controller as unknown as import("../src/views/ssl-settings/acme-certificate-contract").AcmeCertificateController,
      },
      global: {
        stubs: {
          ConfirmDangerPopover: true,
          DropdownMenu: { template: "<div><slot /></div>" },
          DropdownMenuTrigger: { template: "<div><slot /></div>" },
          DropdownMenuContent: { template: "<div><slot /></div>" },
          DropdownMenuSeparator: true,
          DropdownMenuItem: {
            emits: ["select"],
            template: "<button @click=\"$emit('select')\"><slot /></button>",
          },
        },
      },
    });
    wrappers.push(wrapper);
    expect(wrapper.text()).toContain("Issuance succeeded");
    expect(
      wrapper.get('[data-testid="acme-file-output-status"]').text(),
    ).toContain("Permission denied");
    expect(wrapper.text()).toContain("2026/09/01");
    const retry = wrapper
      .findAll("button")
      .find((b) => b.text() === enAdmin.acmeFileOutput.retry)!;
    await retry.trigger("click");
    expect(syncFileOutput).toHaveBeenCalledWith(application);
  });
});
