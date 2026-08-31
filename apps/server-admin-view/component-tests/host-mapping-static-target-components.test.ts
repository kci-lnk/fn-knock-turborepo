import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h, nextTick, reactive, type PropType } from "vue";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";

import { useConfigStore } from "../src/store/config";
import type {
  AppConfig,
  HostMapping,
  HostMappingStaticServe,
} from "../src/types";
import SubdomainMappingRowActions from "../src/views/subdomain-proxy/SubdomainMappingRowActions.vue";
import SubdomainMappingStaticTargetField from "../src/views/subdomain-proxy/SubdomainMappingStaticTargetField.vue";
import SubdomainMappingTargetCell from "../src/views/subdomain-proxy/SubdomainMappingTargetCell.vue";
import SubdomainMappingTargetEditor from "../src/views/subdomain-proxy/SubdomainMappingTargetEditor.vue";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";

const createTestI18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    missingWarn: false,
    fallbackWarn: false,
    messages: {
      en: {
        common: {
          cancel: "Cancel",
          confirm: "Confirm",
          moreActions: "More actions",
        },
        admin: {
          subdomainProxy: {
            advancedAuthConfig: "Advanced authentication",
            clearDefaultDomain: "Clear default",
            deepMonitor: "Deep monitor",
            deepMonitorActive: "Deep monitor active",
            defaultDomainUnavailable: "Default unavailable",
            delete: "Delete",
            disableMapping: "Disable mapping",
            edit: "Edit",
            enableMapping: "Enable mapping",
            moreActions: "More actions",
            moveToGroup: "Move to group",
            paths: "Path rules",
            scheduleAvailability: "Availability",
            setDefaultDomain: "Set default",
            ungrouped: "Ungrouped",
            staticServe: {
              directoryListing: "Directory listing",
              directoryListingHint:
                "Show files when no default document exists.",
              indexFiles: "Default documents",
              indexFilesHint: "Checked in order.",
              indexFilesPlaceholder: "index.html",
              moveIndexDown: "Move down",
              moveIndexUp: "Move up",
              pathDockerHint: "Use a read-only container mount.",
              pathHint: "This path belongs to the gateway server.",
              pathLabel: "Server path",
              browser: { open: "Browse" },
              probeErrors: {
                probe_failed: "Probe failed",
                type_mismatch: "Type mismatch",
              },
              renderReadme: "Render README",
              renderReadmeHint: "Render README below the list.",
              switchConfirmAction: "Switch target",
              switchConfirmDescription: "Proxy-only settings will be cleared.",
              switchConfirmTitle: "Switch target type?",
              targetType: "Response type",
              targetTypeHints: {
                directory: "Serve a directory.",
                file: "Serve a file at the mapping root.",
                proxy: "Forward to an upstream.",
              },
              targetTypes: {
                directory: "Directory",
                file: "Single file",
                proxy: "Reverse proxy",
              },
              validation: {
                duplicate_index_file: "Duplicate default document",
                invalid_index_file: "Invalid default document",
                path_has_parent_segment: "Parent traversal is not allowed",
                path_not_absolute: "Use an absolute path",
                path_required: "Path is required",
                path_unsafe: "Path is unsafe",
                too_many_index_files: "Too many default documents",
              },
            },
          },
        },
      },
    },
  });

const SelectStub = defineComponent({
  emits: ["update:modelValue"],
  setup(_props, { emit }) {
    return () =>
      h("div", { "data-testid": "target-type-select" }, [
        h(
          "button",
          {
            "data-testid": "select-proxy",
            type: "button",
            onClick: () => emit("update:modelValue", "proxy"),
          },
          "proxy",
        ),
        h(
          "button",
          {
            "data-testid": "select-file",
            type: "button",
            onClick: () => emit("update:modelValue", "file"),
          },
          "file",
        ),
        h(
          "button",
          {
            "data-testid": "select-directory",
            type: "button",
            onClick: () => emit("update:modelValue", "directory"),
          },
          "directory",
        ),
      ]);
  },
});

const ConfirmationDialogStub = defineComponent({
  name: "ConfirmationDialog",
  props: { open: Boolean },
  emits: ["confirm", "update:open"],
  setup(props, { emit }) {
    return () =>
      props.open
        ? h("div", { "data-testid": "target-switch-confirmation" }, [
            h(
              "button",
              {
                "data-testid": "cancel-target-switch",
                type: "button",
                onClick: () => emit("update:open", false),
              },
              "Cancel",
            ),
            h(
              "button",
              {
                "data-testid": "confirm-target-switch",
                type: "button",
                onClick: () => emit("confirm"),
              },
              "Confirm",
            ),
          ])
        : null;
  },
});

const StaticTargetFieldStub = defineComponent({
  name: "SubdomainMappingStaticTargetField",
  props: {
    modelValue: {
      type: Object as PropType<HostMappingStaticServe>,
      required: true,
    },
  },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    return () =>
      h("input", {
        "data-testid": "static-target-path",
        value: props.modelValue.path,
        onInput: (event: Event) =>
          emit("update:modelValue", {
            ...props.modelValue,
            path: (event.target as HTMLInputElement).value,
          }),
      });
  },
});

const ProxyTargetFieldStub = defineComponent({
  name: "SubdomainMappingTargetField",
  setup: () => () => h("div", { "data-testid": "proxy-target-field" }),
});

const targetEditorStubs = {
  ConfirmationDialog: ConfirmationDialogStub,
  Select: SelectStub,
  SelectContent: true,
  SelectItem: true,
  SelectTrigger: true,
  SelectValue: true,
  SubdomainMappingStaticTargetField: StaticTargetFieldStub,
  SubdomainMappingTargetField: ProxyTargetFieldStub,
};

const proxyMappingDraft = (): HostMapping => ({
  ...createDefaultMapping(),
  host: "docs.example.test",
  target: "http://docs:8080/base",
  target_path_mode: "prefix",
  basic_auth: {
    enabled: true,
    username: "upstream",
    password: "secret",
  },
  preserve_host: true,
  suppress_toolbar: false,
  locations: [
    {
      path: "/api",
      match: "prefix",
      action: "proxy",
      target: "http://api:8080",
      strip_path: false,
      rewrite_html: false,
      auth_mode: "inherit",
      response: {
        status: 200,
        content_type: "text/plain",
        headers: { "X-Test": "value" },
        body: "",
      },
    },
  ],
});

const mountTargetEditor = (mapping: HostMapping) => {
  const mappingForm = reactive(mapping);
  const wrapper = mount(SubdomainMappingTargetEditor, {
    props: {
      allowTargetPathMode: true,
      mappingForm,
      open: true,
      updateMappingForm: (patch) => Object.assign(mappingForm, patch),
    },
    global: {
      plugins: [createTestI18n()],
      stubs: targetEditorStubs,
    },
  });
  return { mappingForm, wrapper };
};

const SlotStub = defineComponent({
  setup(_props, { slots }) {
    return () => h("div", slots.default?.());
  },
});

const SwitchStub = defineComponent({
  inheritAttrs: false,
  props: {
    disabled: Boolean,
    modelValue: Boolean,
  },
  emits: ["update:modelValue"],
  setup(props, { attrs, emit }) {
    return () =>
      h("button", {
        ...attrs,
        "aria-checked": String(props.modelValue),
        disabled: props.disabled,
        role: "switch",
        type: "button",
        onClick: () => {
          if (!props.disabled) emit("update:modelValue", !props.modelValue);
        },
      });
  },
});

const staticFieldStubs = {
  Switch: SwitchStub,
  TagsInput: SlotStub,
  TagsInputInput: true,
  TagsInputItem: SlotStub,
  TagsInputItemDelete: true,
  TagsInputItemText: true,
};

const staticServeConfig = (
  path: string,
  enabled = true,
  renderReadme = true,
): HostMappingStaticServe => ({
  path,
  index_files: ["index.html", "index.htm"],
  directory_listing: {
    enabled,
    render_readme: renderReadme,
  },
});

const mountStaticTargetField = (modelValue: HostMappingStaticServe) => {
  const pinia = createPinia();
  setActivePinia(pinia);
  const configStore = useConfigStore(pinia);
  configStore.config = {
    runtime_profile: {
      deployment_target: "linux",
      is_docker: false,
      is_linux: true,
      is_root_process: false,
      is_windows: false,
    },
  } as AppConfig;
  return mount(SubdomainMappingStaticTargetField, {
    props: {
      modelValue,
      targetType: "directory",
    },
    global: {
      plugins: [pinia, createTestI18n()],
      stubs: staticFieldStubs,
    },
  });
};

const DropdownItemStub = defineComponent({
  emits: ["select"],
  setup(_props, { emit, slots }) {
    return () =>
      h(
        "button",
        { type: "button", onClick: () => emit("select") },
        slots.default?.(),
      );
  },
});

const dropdownStubs = {
  DropdownMenu: SlotStub,
  DropdownMenuContent: SlotStub,
  DropdownMenuItem: DropdownItemStub,
  DropdownMenuSeparator: true,
  DropdownMenuSub: SlotStub,
  DropdownMenuSubContent: SlotStub,
  DropdownMenuSubTrigger: SlotStub,
  DropdownMenuTrigger: SlotStub,
};

const findButton = (wrapper: ReturnType<typeof mount>, label: string) =>
  wrapper.findAll("button").find((button) => button.text().includes(label));

describe("host mapping static target components", () => {
  it("requires confirmation before clearing proxy-only settings", async () => {
    const { mappingForm, wrapper } = mountTargetEditor(proxyMappingDraft());

    await wrapper.get('[data-testid="select-file"]').trigger("click");
    await nextTick();
    expect(
      wrapper.find('[data-testid="target-switch-confirmation"]').exists(),
    ).toBe(true);

    await wrapper.get('[data-testid="cancel-target-switch"]').trigger("click");
    await flushPromises();
    expect(mappingForm.target_type).toBe("proxy");
    expect(mappingForm.target).toBe("http://docs:8080/base");
    expect(mappingForm.locations).toHaveLength(1);

    await wrapper.get('[data-testid="select-directory"]').trigger("click");
    await wrapper.get('[data-testid="confirm-target-switch"]').trigger("click");
    await flushPromises();
    expect(mappingForm.target_type).toBe("directory");
    expect(mappingForm.target).toBe("");
    expect(mappingForm.locations).toEqual([]);
    expect(mappingForm.basic_auth.enabled).toBe(false);
    expect(mappingForm.preserve_host).toBe(false);
    expect(mappingForm.target_path_mode).toBe("entry");
    expect(wrapper.find('[data-testid="proxy-target-field"]').exists()).toBe(
      false,
    );
    expect(wrapper.find('[data-testid="static-target-path"]').exists()).toBe(
      true,
    );
  });

  it("restores independent proxy, file, and directory drafts while the dialog stays open", async () => {
    const original = proxyMappingDraft();
    const { mappingForm, wrapper } = mountTargetEditor(original);

    await wrapper.get('[data-testid="select-file"]').trigger("click");
    await wrapper.get('[data-testid="confirm-target-switch"]').trigger("click");
    await flushPromises();
    await wrapper
      .get('[data-testid="static-target-path"]')
      .setValue("/srv/manual.pdf");

    await wrapper.get('[data-testid="select-directory"]').trigger("click");
    await flushPromises();
    await wrapper
      .get('[data-testid="static-target-path"]')
      .setValue("/srv/documentation");

    await wrapper.get('[data-testid="select-file"]').trigger("click");
    await flushPromises();
    expect(mappingForm.static_serve?.path).toBe("/srv/manual.pdf");

    await wrapper.get('[data-testid="select-directory"]').trigger("click");
    await flushPromises();
    expect(mappingForm.static_serve?.path).toBe("/srv/documentation");

    await wrapper.get('[data-testid="select-proxy"]').trigger("click");
    await flushPromises();
    expect(mappingForm.target).toBe("http://docs:8080/base");
    expect(mappingForm.target_path_mode).toBe("prefix");
    expect(mappingForm.basic_auth).toEqual({
      enabled: true,
      username: "upstream",
      password: "secret",
    });
    expect(mappingForm.locations).toHaveLength(1);
    expect(mappingForm.preserve_host).toBe(true);
    expect(mappingForm.suppress_toolbar).toBe(false);
  });

  it("preserves a trailing-space POSIX path when opening the browser", async () => {
    const path = "/srv/docs ";
    const wrapper = mountStaticTargetField(staticServeConfig(path));

    await wrapper.get('[data-testid="browse-static-path"]').trigger("click");

    expect(wrapper.emitted("browse")).toEqual([["directory", path]]);
    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    expect(wrapper.text()).not.toContain("Check path");
  });

  it("turning directory listing off clears and disables README rendering", async () => {
    const wrapper = mountStaticTargetField(staticServeConfig("/srv/docs"));
    const listing = wrapper.get("#mapping-directory-listing");
    const readme = wrapper.get("#mapping-render-readme");
    expect(listing.attributes("aria-checked")).toBe("true");
    expect(readme.attributes("aria-checked")).toBe("true");
    expect(readme.attributes()).not.toHaveProperty("disabled");

    await listing.trigger("click");
    const update = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as
      HostMappingStaticServe | undefined;
    expect(update?.directory_listing).toEqual({
      enabled: false,
      render_readme: false,
    });
    await wrapper.setProps({ modelValue: update });
    expect(wrapper.get("#mapping-render-readme").attributes()).toHaveProperty(
      "disabled",
    );
    expect(
      wrapper.get("#mapping-render-readme").attributes("aria-checked"),
    ).toBe("false");
  });

  it("shows the static type badge and administrator-visible server path", async () => {
    const directory = {
      ...createDefaultMapping(),
      host: "docs.example.test",
      target_type: "directory" as const,
      target: "http://stale-upstream:8080",
      static_serve: staticServeConfig("/srv/private-docs"),
    };
    const wrapper = mount(SubdomainMappingTargetCell, {
      props: { mapping: directory, unavailable: false },
      global: { plugins: [createTestI18n()] },
    });
    expect(wrapper.text()).toContain("/srv/private-docs");
    expect(wrapper.text()).toContain("Directory");
    expect(wrapper.text()).not.toContain("stale-upstream");

    await wrapper.setProps({
      mapping: {
        ...directory,
        target_type: "file",
        static_serve: staticServeConfig("/srv/manual.pdf", false, false),
      },
    });
    expect(wrapper.text()).toContain("/srv/manual.pdf");
    expect(wrapper.text()).toContain("Single file");

    await wrapper.setProps({ asCell: false, compact: true });
    expect(wrapper.element.tagName).toBe("DIV");
    expect(wrapper.get("span[title]").classes()).toContain("truncate");
  });

  it("hides proxy-only row actions for static mappings and keeps shared actions", async () => {
    const mapping: HostMapping = {
      ...createDefaultMapping(),
      host: "docs.example.test",
      target_type: "directory",
      target: "",
      static_serve: staticServeConfig("/srv/docs"),
      use_auth: true,
    };
    const wrapper = mount(SubdomainMappingRowActions, {
      props: {
        canUseDeepMonitor: true,
        deepMonitorActive: false,
        groups: [{ id: "ops", name: "Operations" }],
        isAuthServiceTarget: () => false,
        isDefaultDomainAvailable: true,
        isSavingMappings: false,
        mapping,
      },
      global: {
        plugins: [createTestI18n()],
        stubs: dropdownStubs,
      },
    });

    expect(wrapper.text()).not.toContain("Path rules");
    for (const label of [
      "Deep monitor",
      "Advanced authentication",
      "Set default",
      "Disable mapping",
      "Move to group",
      "Operations",
      "Availability",
      "Delete",
    ]) {
      expect(wrapper.text()).toContain(label);
    }

    await findButton(wrapper, "Deep monitor")?.trigger("click");
    await findButton(wrapper, "Advanced authentication")?.trigger("click");
    await findButton(wrapper, "Operations")?.trigger("click");
    await findButton(wrapper, "Availability")?.trigger("click");
    expect(wrapper.emitted("open-deep-monitor")).toEqual([
      ["docs.example.test"],
    ]);
    expect(wrapper.emitted("open-advanced-auth")).toEqual([
      ["docs.example.test"],
    ]);
    expect(wrapper.emitted("move")).toEqual([[mapping, "ops"]]);
    expect(wrapper.emitted("open-availability")).toEqual([[mapping]]);
    expect(wrapper.emitted("open-gateway-locations")).toBeUndefined();

    await wrapper.setProps({
      asCell: false,
      compact: true,
      triggerAriaLabel: "More actions: docs.example.test",
    });
    expect(wrapper.element.tagName).toBe("DIV");
    expect(
      wrapper.get('button[aria-label="More actions: docs.example.test"]'),
    ).toBeDefined();
    await findButton(wrapper, "Edit")?.trigger("click");
    expect(wrapper.emitted("edit")).toEqual([[mapping]]);
  });
});
