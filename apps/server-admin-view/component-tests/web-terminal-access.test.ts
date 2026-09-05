import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createI18n } from "vue-i18n";
import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

const api = vi.hoisted(() => ({
  settings: vi.fn(),
  update: vi.fn(),
  status: vi.fn(),
  verify: vi.fn(),
}));
const push = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api/terminal-access", async (original) => ({
  ...(await original<typeof import("@/lib/api/terminal-access")>()),
  TerminalAccessAPI: api,
}));
vi.mock("vue-router", () => ({ useRouter: () => ({ push }) }));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));
vi.mock("@/views/web-terminal/WebTerminalAuthorized.vue", () => ({
  default: { template: '<div data-test="workspace" />' },
}));

import WebTerminal from "@/views/WebTerminal.vue";
import WebTerminalSettings from "@/views/system-settings/WebTerminalSettings.vue";
import { useTerminalAccessStore } from "@/store/terminal-access";
import { Switch } from "@/components/ui/switch";

const configured = { enabled: true, passwordConfigured: true, revision: "one" };
const pendingAccess = { ...configured, authorized: false };
const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: { en: { admin: enAdmin } },
  });
function options() {
  return {
    global: {
      plugins: [createPinia(), i18n()],
      stubs: {
        FloatingActionDock: { template: '<div><slot name="inline" /></div>' },
        RefreshButton: {
          template:
            '<button type="button" @click="$emit(\'click\')">Refresh</button>',
        },
      },
    },
  };
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  setActivePinia(createPinia());
  api.settings.mockResolvedValue(configured);
  api.status.mockResolvedValue(pendingAccess);
  api.verify.mockResolvedValue(undefined);
});
afterEach(() => {
  vi.useRealTimers();
});

describe("Web Terminal access", () => {
  it("mounts only after successful verification and locks on revocation", async () => {
    const wrapper = mount(WebTerminal, options());
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    await flushPromises();
    expect(wrapper.find('input[type="password"]').exists()).toBe(true);
    api.verify.mockRejectedValueOnce({
      response: { data: { errorCode: "access_password_required" } },
    });
    await wrapper.get("input").setValue("wrong");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain(
      "Incorrect password",
    );
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    api.status.mockResolvedValue({ ...pendingAccess, authorized: true });
    await wrapper.get("input").setValue("correct");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.verify).toHaveBeenLastCalledWith("correct");
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(true);
    api.status.mockResolvedValue({ ...pendingAccess, revision: "two" });
    await vi.advanceTimersByTimeAsync(5000);
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    wrapper.unmount();
  });
  it("reuses server authorization on reentry and redirects when disabled", async () => {
    api.status.mockResolvedValue({ ...pendingAccess, authorized: true });
    const wrapper = mount(WebTerminal, options());
    await flushPromises();
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(true);
    expect(api.verify).not.toHaveBeenCalled();
    api.status.mockResolvedValue({ ...pendingAccess, enabled: false });
    await vi.advanceTimersByTimeAsync(5000);
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    expect(push).toHaveBeenCalledWith({
      path: "/system",
      query: { tab: "features" },
    });
    wrapper.unmount();
  });
  it("fails closed when the access check fails", async () => {
    api.status.mockRejectedValue(new Error("offline"));
    const wrapper = mount(WebTerminal, options());
    useTerminalAccessStore().applySettings({
      ...configured,
      passwordConfigured: false,
    });
    await flushPromises();
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    expect(wrapper.get('[role="alert"]').text()).toContain("Request failed");
    wrapper.unmount();
  });
  it("ignores a stale status response after settings change", async () => {
    let resolve!: (value: typeof pendingAccess) => void;
    api.status.mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const access = useTerminalAccessStore();
    const refresh = access.refresh();
    access.applySettings({ ...configured, enabled: false, revision: "two" });
    resolve({ ...pendingAccess, authorized: true });
    await refresh;
    expect(access.status?.enabled).toBe(false);
    expect(access.status?.revision).toBe("two");
  });
});

describe("Web Terminal settings", () => {
  it("hides password configuration while disabled and preserves it on save", async () => {
    const wrapper = mount(WebTerminalSettings, options());
    await flushPromises();
    expect(wrapper.text()).toContain("Configured");
    expect(
      (wrapper.get('input[type="password"]').element as HTMLInputElement).value,
    ).toBe("");
    wrapper.getComponent(Switch).vm.$emit("update:modelValue", false);
    await nextTick();
    expect(wrapper.find('input[type="password"]').exists()).toBe(false);
    api.update.mockResolvedValue({
      ...configured,
      enabled: false,
      revision: "two",
    });
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.update).toHaveBeenCalledWith({
      enabled: false,
      revision: "one",
      password: undefined,
      clearPassword: false,
    });
    expect(useTerminalAccessStore().status?.enabled).toBe(false);
    wrapper.unmount();
  });
  it("retains a failed password draft and supports explicit clearing", async () => {
    const wrapper = mount(WebTerminalSettings, options());
    await flushPromises();
    await wrapper.get('input[type="password"]').setValue("new-secret");
    api.update.mockRejectedValueOnce(new Error("offline"));
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(
      (wrapper.get('input[type="password"]').element as HTMLInputElement).value,
    ).toBe("new-secret");
    expect(wrapper.get('[role="alert"]').text()).toContain("Request failed");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Clear password")!
      .trigger("click");
    expect(wrapper.text()).toContain("will be cleared");
    api.update.mockResolvedValue({
      ...configured,
      passwordConfigured: false,
      revision: "two",
    });
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.update).toHaveBeenLastCalledWith({
      enabled: true,
      revision: "one",
      password: undefined,
      clearPassword: true,
    });
    expect(wrapper.text()).toContain("Not configured");
    wrapper.unmount();
  });
});

it("successful verification supersedes an older pending access poll", async () => {
  const wrapper = mount(WebTerminal, options());
  await flushPromises();
  let resolve!: (value: typeof pendingAccess) => void;
  api.status.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  vi.advanceTimersByTime(5000);
  api.status.mockResolvedValue({ ...pendingAccess, authorized: true });
  await wrapper.get("input").setValue("correct");
  await wrapper.get("form").trigger("submit");
  await flushPromises();
  expect(wrapper.find('[data-test="workspace"]').exists()).toBe(true);
  resolve(pendingAccess);
  await flushPromises();
  expect(wrapper.find('[data-test="workspace"]').exists()).toBe(true);
  wrapper.unmount();
});

it("a failed newer access check invalidates cached authorization despite an older successful response", async () => {
  const access = useTerminalAccessStore();
  access.applySettings({ ...configured, passwordConfigured: false });
  let resolveOld!: (value: typeof pendingAccess) => void;
  api.status.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
  );
  const old = access.refresh();
  api.status.mockRejectedValueOnce(new Error("offline"));
  await expect(access.refresh()).rejects.toThrow("offline");
  resolveOld({ ...pendingAccess, authorized: true });
  await old;
  expect(access.status?.authorized).toBe(false);
});

it("does not let an old failed access check revoke a newer successful verification", async () => {
  const access = useTerminalAccessStore();
  let rejectOld!: (error: Error) => void;
  api.status.mockImplementationOnce(
    () =>
      new Promise((_, reject) => {
        rejectOld = reject;
      }),
  );
  const old = access.refresh().catch(() => undefined);
  api.status.mockResolvedValueOnce({ ...pendingAccess, authorized: true });
  await access.refresh();
  rejectOld(new Error("old failure"));
  await old;
  expect(access.status?.authorized).toBe(true);
});

it("coalesces repeated settings refreshes and ignores responses after leaving", async () => {
  let resolve!: (value: typeof configured) => void;
  api.settings.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const wrapper = mount(WebTerminalSettings, options());
  const access = useTerminalAccessStore();
  await wrapper
    .findAll("button")
    .find((button) => button.text() === "Refresh")!
    .trigger("click");
  expect(api.settings).toHaveBeenCalledTimes(1);
  wrapper.unmount();
  access.applySettings({ ...configured, enabled: false, revision: "new" });
  resolve(configured);
  await flushPromises();
  expect(access.status?.enabled).toBe(false);
  expect(access.status?.revision).toBe("new");
});

it("does not publish an unmounted settings page's late save over newer state", async () => {
  const wrapper = mount(WebTerminalSettings, options());
  await flushPromises();
  let resolve!: (value: typeof configured) => void;
  api.update.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  await wrapper.get('input[type="password"]').setValue("new-secret");
  await wrapper.get("form").trigger("submit");
  const access = useTerminalAccessStore();
  wrapper.unmount();
  access.applySettings({ ...configured, enabled: false, revision: "latest" });
  resolve({ ...configured, revision: "older-save" });
  await flushPromises();
  expect(access.status?.enabled).toBe(false);
  expect(access.status?.revision).toBe("latest");
});
