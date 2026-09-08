import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MaintenanceAPI } from "../src/lib/api/config";
import AutomaticBackupSettings from "../src/views/system-settings/AutomaticBackupSettings.vue";
import { defaultBackupEmail } from "../src/lib/backup-email";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}));
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
  vi.useRealTimers();
});
async function setup() {
  const details = {
    config: {
      enabled: true,
      interval_hours: 24,
      retention_days: 7,
      updated_at: null,
      email: { ...defaultBackupEmail(), password_configured: true },
    },
    status: {
      directory_path: "/backups",
      last_attempt_at: null,
      last_success_at: "2026-09-08T00:00:00Z",
      last_error: null,
      last_filename: null,
      next_backup_at: null,
      email: {
        last_attempt_at: null,
        last_success_at: null,
        last_filename: "sample.knock",
        last_error: "smtp_timeout",
        pending_count: 1,
        next_retry_at: null,
      },
    },
  };
  vi.spyOn(MaintenanceAPI, "getAutomaticBackupDetails").mockResolvedValue(
    details,
  );
  vi.spyOn(MaintenanceAPI, "updateAutomaticBackupConfig").mockResolvedValue(
    details,
  );
  vi.spyOn(MaintenanceAPI, "testBackupEmail").mockResolvedValue(undefined);
  const wrapper = mount(AutomaticBackupSettings, {
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
  wrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}
function button(wrapper: ReturnType<typeof mount>, text: string) {
  return wrapper.findAll("button").find((value) => value.text() === text)!;
}
describe("backup email settings", () => {
  it("keeps saved credentials private and resets draft edits", async () => {
    const wrapper = await setup();
    const input = wrapper.get('input[type="password"]');
    expect((input.element as HTMLInputElement).value).toBe("");
    expect(input.attributes("placeholder")).toBe(
      enAdmin.maintenanceSettings.emailPasswordSaved,
    );
    await input.setValue("new password");
    await button(wrapper, enAdmin.maintenanceSettings.resetAutomatic).trigger(
      "click",
    );
    expect((input.element as HTMLInputElement).value).toBe("");
    expect(MaintenanceAPI.updateAutomaticBackupConfig).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain(
      enAdmin.maintenanceSettings.emailErrorTimeout,
    );
    expect(wrapper.text()).toContain(
      enAdmin.maintenanceSettings.automaticLastSuccess,
    );
  });
  it("submits password changes only when explicitly entered", async () => {
    const wrapper = await setup();
    await wrapper.get('input[type="password"]').setValue("replacement");
    await button(wrapper, enAdmin.maintenanceSettings.saveAutomatic).trigger(
      "click",
    );
    await flushPromises();
    const call = vi.mocked(MaintenanceAPI.updateAutomaticBackupConfig).mock
      .calls[0]![0];
    expect(call.email?.password).toBe("replacement");
    expect(call.email).not.toHaveProperty("password_configured");
  });
  it("tests the current draft without saving it", async () => {
    const wrapper = await setup();
    await wrapper.get('input[id$="-host"]').setValue("smtp.example.com");
    await wrapper.get('input[id$="-from"]').setValue("backup@example.com");
    await wrapper
      .get('input[id$="-to"]')
      .setValue("one@example.com, two@example.com");
    await wrapper.get('select[id$="-auth"]').setValue("none");
    await button(wrapper, enAdmin.maintenanceSettings.emailTest).trigger(
      "click",
    );
    await flushPromises();
    expect(MaintenanceAPI.testBackupEmail).toHaveBeenCalledOnce();
    expect(MaintenanceAPI.updateAutomaticBackupConfig).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain(
      enAdmin.maintenanceSettings.emailTestSuccess,
    );
  });
  it("ignores an old status poll that completes after saving", async () => {
    vi.useFakeTimers();
    const wrapper = await setup();
    const original = await MaintenanceAPI.getAutomaticBackupDetails();
    let resolvePoll!: (value: typeof original) => void;
    vi.mocked(MaintenanceAPI.getAutomaticBackupDetails).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolvePoll = resolve;
        }),
    );
    await vi.advanceTimersByTimeAsync(5000);
    vi.mocked(MaintenanceAPI.updateAutomaticBackupConfig).mockResolvedValueOnce(
      {
        ...original,
        status: {
          ...original.status,
          email: {
            ...original.status.email!,
            last_filename: "saved-file.knock",
          },
        },
      },
    );
    await wrapper.get('input[type="password"]').setValue("replacement");
    await button(wrapper, enAdmin.maintenanceSettings.saveAutomatic).trigger(
      "click",
    );
    await flushPromises();
    resolvePoll({
      ...original,
      status: {
        ...original.status,
        email: { ...original.status.email!, last_filename: "stale-file.knock" },
      },
    });
    await flushPromises();
    expect(wrapper.text()).toContain("saved-file.knock");
    expect(wrapper.text()).not.toContain("stale-file.knock");
  });
});
