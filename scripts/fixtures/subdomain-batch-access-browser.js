import { createApp, h, computed, ref } from "vue";
import { createI18n } from "vue-i18n";
import { messages } from "@fn-knock/i18n";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { createDefaultMapping } from "/src/views/subdomain-proxy/model.ts";
import { useSubdomainBatchActions } from "/src/views/subdomain-proxy/useSubdomainBatchActions.ts";
import SubdomainMappingsCard from "/src/views/subdomain-proxy/SubdomainMappingsCard.vue";
import SubdomainActionConfirmDialog from "/src/views/subdomain-proxy/SubdomainActionConfirmDialog.vue";
import "/src/assets/index.css";

const query = new URLSearchParams(location.search);
const locale = query.get("locale") || "en";
const grouped = query.get("grouped") === "true";
document.documentElement.classList.toggle("dark", query.get("dark") === "true");
const i18n = createI18n({ legacy: false, locale, messages });
const t = i18n.global.t;
const noop = () => {};
const isAuthServiceTarget = (target) => target === "http://auth:7997";
createApp({
  setup() {
    const mappings = ref(
      ["one.test", "two.test"].map((host, index) => ({
        ...createDefaultMapping(),
        host,
        target: "http://127.0.0.1:8080",
        title: index ? "Documentation" : "Workspace",
        use_auth: index === 0,
        group_id: grouped ? "team" : null,
      })),
    );
    if (query.get("protectedAuth") === "true") {
      mappings.value.push(
        {
          ...createDefaultMapping(),
          host: "auth-role.test",
          title: "Auth role",
          service_role: "auth",
          target: "http://auth:8000",
          use_auth: false,
        },
        {
          ...createDefaultMapping(),
          host: "auth-target.test",
          title: "Auth target",
          target: "http://auth:7997",
          use_auth: false,
        },
      );
    }
    let writes = 0;
    let failure = false;
    let release;
    let pending;
    const errors = [];
    const { isPending, run } = useAsyncAction({
      onError: (error) => errors.push(error.message),
    });
    const actions = useSubdomainBatchActions({
      allMappings: computed(() => mappings.value),
      isAuthServiceTarget,
      isSavingMappings: isPending,
      runSaveMappings: run,
      translate: t,
      saveHostMappings: async (next, beforeSave) => {
        beforeSave();
        writes++;
        await pending;
        if (failure) throw new Error("Save failed");
        mappings.value = next;
      },
    });
    window.batchTest = {
      labels: {
        select: t("admin.subdomainProxy.selectionMode"),
        all: t("admin.subdomainProxy.selectAllMappings"),
        cancel: t("admin.subdomainProxy.cancel"),
        title: t("admin.subdomainProxy.batchPublicAccessTitle", { count: 2 }),
      },
      snapshot: () => ({
        writes,
        errors,
        auth: mappings.value.map((m) => m.use_auth),
        open: actions.isBatchMutationOpen.value,
      }),
      fail: (value) => {
        failure = value;
      },
      hold: () => {
        pending = new Promise((resolve) => {
          release = resolve;
        });
      },
      release: () => {
        release?.();
        pending = undefined;
      },
    };
    return () =>
      h(
        "main",
        { class: "min-h-screen bg-background p-3 text-foreground sm:p-6" },
        [
          h(SubdomainMappingsCard, {
            activeDeepMonitorHosts: [],
            allRegularMappings: mappings.value,
            authServiceMapping: null,
            canManageNewMappings: true,
            canUseDeepMonitor: false,
            discoverButtonDividerClass: "",
            discoverButtonVariant: "outline",
            docsHref: "#docs",
            draggableMappings: mappings.value,
            filteredMappings: mappings.value,
            formatHost: (host) => host,
            formatAvailabilityWindow: () => "",
            getAvailabilityState: () => "available",
            getHostTrafficSample: () => null,
            getMappingTitleForDisplay: (mapping) => mapping.title,
            globalVisibilityEnabled: false,
            globalWafEnabled: false,
            groupedView: grouped,
            groups: grouped ? [{ id: "team", name: "Team services" }] : [],
            handleMappingStatusTooltipOpenChange: noop,
            handleMappingStatusTooltipTriggerClick: noop,
            handleProtocolHeadersWarningOpenChange: noop,
            hasRegularHostMappings: true,
            isClearingAllSubdomainConfig: false,
            isConfigLoading: false,
            isDiscovering: false,
            isExportingBookmarks: false,
            isFaviconBroken: () => true,
            isGatewayPortalEnabled: false,
            isDefaultDomainAvailable: true,
            isMappingUnavailable: () => false,
            isMappingStatusTooltipOpen: () => false,
            isProtocolHeadersWarningOpen: () => false,
            isRefreshingTitles: false,
            isRootDomainPendingSave: false,
            isSavingMappings: isPending.value,
            isSyncing: false,
            isAuthServiceTarget,
            markFaviconBroken: noop,
            openProtocolHeadersWarning: noop,
            rootDomainValidationMessage: "",
            savedRootDomain: "example.com",
            scheduleCloseProtocolHeadersWarning: noop,
            searchQuery: "",
            shouldShowProtocolHeadersWarning: () => false,
            toggleProtocolHeadersWarning: noop,
            trafficTimestamp: null,
            visibleMappingsCount: mappings.value.length,
            onBatchRequireAuth: (hosts, done) =>
              actions.openBatchMutation(hosts, "require-auth", done),
            onBatchPublicAccess: (hosts, done) =>
              actions.openBatchMutation(hosts, "public-access", done),
          }),
          h(SubdomainActionConfirmDialog, {
            open: actions.isBatchMutationOpen.value,
            loading: isPending.value,
            title: actions.batchMutationTitle.value,
            description: actions.batchMutationDescription.value,
            cancelLabel: t("admin.subdomainProxy.cancel"),
            confirmLabel: actions.batchMutationConfirmLabel.value,
            confirmVariant: actions.batchMutationConfirmVariant.value,
            "onUpdate:open": (open) => {
              if (!open) actions.closeBatchMutation();
            },
            onCancel: actions.closeBatchMutation,
            onConfirm: actions.confirmBatchMutation,
          }),
        ],
      );
  },
})
  .use(i18n)
  .mount("#app");
