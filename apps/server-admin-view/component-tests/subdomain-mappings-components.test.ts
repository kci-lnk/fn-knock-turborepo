import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";

import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import SubdomainMappingTitleCell from "../src/views/subdomain-proxy/SubdomainMappingTitleCell.vue";
import SubdomainMappingsBatchActions from "../src/views/subdomain-proxy/SubdomainMappingsBatchActions.vue";
import SubdomainMappingsCardHeader from "../src/views/subdomain-proxy/SubdomainMappingsCardHeader.vue";

const createTestI18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        common: {
          docs: "Docs",
          moreActions: "More actions",
        },
        admin: {
          subdomainProxy: {
            addAuthService: "Add auth service",
            batchActions: "Batch actions",
            clearSelection: "Clear selection",
            edit: "Edit",
            editMappingAria: "Edit {host}",
            groupedView: "Grouped view",
            manageGroups: "Manage groups",
            mappingsDescription: "Manage mappings",
            mappingsTitle: "Mappings",
            moveToGroup: "Move to group",
            selectedMappingsCount: "{count} selected",
            ungrouped: "Ungrouped",
          },
        },
      },
    },
  });

const headerProps = {
  allMappingsCount: 0,
  authServiceMapping: null,
  canManageNewMappings: true,
  discoverButtonDividerClass: "border-primary",
  discoverButtonVariant: "default" as const,
  docsHref: "https://example.com/docs",
  groupedView: false,
  hasRegularHostMappings: false,
  isClearingAllSubdomainConfig: false,
  isConfigLoading: false,
  isDiscovering: false,
  isExportingBookmarks: false,
  isRefreshingTitles: false,
  isSavingMappings: false,
  isSyncing: false,
  visibleMappingsCount: 0,
};

describe("subdomain mapping presentation components", () => {
  it("keeps grouped-view and group-management actions independently typed", async () => {
    const wrapper = mount(SubdomainMappingsCardHeader, {
      props: headerProps,
      global: { plugins: [createTestI18n()] },
    });

    const groupedViewButton = wrapper
      .findAll("button")
      .find((button) => button.text().includes("Grouped view"));
    expect(groupedViewButton).toBeDefined();
    await groupedViewButton?.trigger("click");
    expect(wrapper.emitted("update-grouped-view")).toEqual([[true]]);

    await wrapper.setProps({ groupedView: true });
    const manageGroupsButton = wrapper
      .findAll("button")
      .find((button) => button.text().includes("Manage groups"));
    expect(manageGroupsButton).toBeDefined();
    await manageGroupsButton?.trigger("click");
    expect(wrapper.emitted("manage-groups")).toHaveLength(1);
  });

  it("disables header mutations while mappings are being saved", () => {
    const wrapper = mount(SubdomainMappingsCardHeader, {
      props: {
        ...headerProps,
        groupedView: true,
        isSavingMappings: true,
      },
      global: { plugins: [createTestI18n()] },
    });

    for (const label of ["Grouped view", "Manage groups", "Add auth service"]) {
      const button = wrapper
        .findAll("button")
        .find((candidate) => candidate.text().includes(label));
      expect(button?.attributes()).toHaveProperty("disabled");
    }
  });

  it("emits batch clear and row edit actions without leaking parent state", async () => {
    const batch = mount(SubdomainMappingsBatchActions, {
      props: {
        groups: [{ id: "internal", name: "Internal" }],
        saving: false,
        selectedCount: 2,
      },
      global: { plugins: [createTestI18n()] },
    });
    const clearButton = batch
      .findAll("button")
      .find((button) => button.text().includes("Clear selection"));
    await clearButton?.trigger("click");
    expect(batch.emitted("clear")).toHaveLength(1);
    expect(batch.classes()).toContain("grid");
    expect(batch.classes()).toContain("grid-cols-2");
    expect(
      batch
        .findAll("button")
        .filter((button) => button.classes().includes("h-10")),
    ).toHaveLength(6);

    const mapping = {
      ...createDefaultMapping(),
      host: "demo.example.com",
      target: "http://demo:8080/",
      title: "Demo",
    };
    const titleCell = mount(SubdomainMappingTitleCell, {
      props: {
        deepMonitorActive: false,
        formatHost: (host) => host,
        getMappingTitleForDisplay: (item) => item.title,
        handleProtocolHeadersWarningOpenChange: () => undefined,
        isProtocolHeadersWarningOpen: () => false,
        mapping,
        openProtocolHeadersWarning: () => undefined,
        scheduleCloseProtocolHeadersWarning: () => undefined,
        shouldShowProtocolHeadersWarning: () => false,
        toggleProtocolHeadersWarning: () => undefined,
      },
      global: { plugins: [createTestI18n()] },
    });
    await titleCell.get('[data-affordance="edit"]').trigger("click");
    expect(titleCell.emitted("edit")).toEqual([[mapping]]);
  });
});
