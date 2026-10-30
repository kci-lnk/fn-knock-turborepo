import { flushPromises, mount, shallowMount } from "@vue/test-utils";
import { reactive } from "vue";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import {
  PanelSyncAPI,
  type PanelConnection,
  type PanelProviderDescriptor,
} from "../src/lib/api/panel-sync-api";
import PanelSync from "../src/views/PanelSync.vue";
import ConnectionEditor from "../src/views/panel-sync/ConnectionEditor.vue";
import ConnectionList from "../src/views/panel-sync/ConnectionList.vue";
import { createPanelSyncForm } from "../src/views/panel-sync/panel-sync-model";

vi.mock("@admin-shared/utils/toast", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const providers: PanelProviderDescriptor[] = [
  {
    provider: "sun_panel",
    name: "Sun-Panel",
    default_api_path: "/openapi/v1",
    supports_icon: true,
    supports_delete: false,
    notes: [],
  },
  {
    provider: "van_nav",
    name: "Van-Nav",
    default_api_path: "/api",
    supports_icon: true,
    supports_delete: true,
    notes: [],
  },
  {
    provider: "one_nav",
    name: "OneNav",
    default_api_path: "/index.php?c=api",
    supports_icon: false,
    supports_delete: true,
    notes: [],
  },
];
const connection: PanelConnection = {
  id: "panel-1",
  name: "NAS",
  provider: "sun_panel",
  base_url: "https://nav.example.test",
  api_path: "/openapi/v1",
  sync_icons: false,
  allow_invalid_tls: false,
  grouping: { mode: "mirror", namespace: "fn-knock", single_group_name: "" },
  auto_sync: { enabled: false, interval_minutes: 60 },
  credential_configured: true,
  verified_at: "2026-10-31T00:00:00Z",
  verified_version: null,
  created_at: "",
  updated_at: "",
  last_run: null,
  next_sync_at: null,
};
const i18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        admin: enAdmin,
        common: { save: "Save", refreshStatus: "Refresh" },
      },
    },
  });
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
});

describe("panel icon sync settings", () => {
  it("shows a labelled switch only for providers with icon support", async () => {
    const form = reactive(createPanelSyncForm());
    const wrapper = mount(ConnectionEditor, {
      props: {
        form,
        providers,
        open: true,
        autoSyncReady: true,
        draftVerified: false,
        isEditing: false,
        saving: false,
        testing: false,
      },
      global: {
        plugins: [i18n()],
        stubs: {
          Dialog: { template: "<div><slot /></div>" },
          DialogContent: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<h2><slot /></h2>" },
          DialogDescription: { template: "<p><slot /></p>" },
          ProviderPicker: true,
        },
      },
    });
    wrappers.push(wrapper);
    const iconSwitch = () => wrapper.get('[role="switch"][id$="-sync-icons"]');
    expect(iconSwitch().attributes("aria-checked")).toBe("true");
    const switchId = iconSwitch().attributes("id");
    expect(wrapper.get(`label[for="${switchId}"]`).text()).toBe("Sync icons");
    expect(
      wrapper
        .get(`[id="${iconSwitch().attributes("aria-describedby")}"]`)
        .text(),
    ).toBe(enAdmin.panelSync.editor.syncIconsHint);
    await iconSwitch().trigger("click");
    expect(form.sync_icons).toBe(false);
    form.provider = "one_nav";
    await flushPromises();
    expect(wrapper.find('[id$="-sync-icons"]').exists()).toBe(false);
    form.provider = "van_nav";
    await flushPromises();
    expect(iconSwitch().attributes("aria-checked")).toBe("false");
    expect(wrapper.text()).toContain(
      enAdmin.panelSync.editor.syncIconsAutoFetchHint,
    );
  });

  it("preserves disabled icon syncing when toggling automation, testing and saving an edit", async () => {
    vi.spyOn(PanelSyncAPI, "providers").mockResolvedValue(providers);
    vi.spyOn(PanelSyncAPI, "connections").mockResolvedValue([connection]);
    const update = vi
      .spyOn(PanelSyncAPI, "update")
      .mockResolvedValue(connection);
    const testDraft = vi.spyOn(PanelSyncAPI, "testDraft").mockResolvedValue({
      success: true,
      provider: "sun_panel",
      version: null,
      message: "OK",
      capabilities: {
        can_create: true,
        can_update: true,
        can_update_groups: false,
        can_delete: false,
        supports_icon: true,
        residual_on_delete: true,
      },
    });
    vi.spyOn(PanelSyncAPI, "testSaved").mockResolvedValue({
      success: true,
      provider: "sun_panel",
      version: null,
      message: "OK",
      capabilities: {
        can_create: true,
        can_update: true,
        can_update_groups: false,
        can_delete: false,
        supports_icon: true,
        residual_on_delete: true,
      },
    });
    const wrapper = shallowMount(PanelSync, { global: { plugins: [i18n()] } });
    wrappers.push(wrapper);
    await flushPromises();
    wrapper
      .getComponent(ConnectionList)
      .vm.$emit("toggle-auto", connection, true);
    await flushPromises();
    expect(update).toHaveBeenLastCalledWith(
      connection.id,
      expect.objectContaining({
        sync_icons: false,
        auto_sync: { enabled: true, interval_minutes: 60 },
      }),
    );
    wrapper.getComponent(ConnectionList).vm.$emit("edit", connection);
    await flushPromises();
    const editor = wrapper.getComponent(ConnectionEditor);
    expect(editor.props("form").sync_icons).toBe(false);
    editor.vm.$emit("test");
    await flushPromises();
    expect(testDraft).toHaveBeenCalledWith(
      expect.objectContaining({ sync_icons: false }),
      connection.id,
    );
    editor.vm.$emit("save");
    await flushPromises();
    expect(update).toHaveBeenLastCalledWith(
      connection.id,
      expect.objectContaining({ sync_icons: false }),
    );
  });
});
