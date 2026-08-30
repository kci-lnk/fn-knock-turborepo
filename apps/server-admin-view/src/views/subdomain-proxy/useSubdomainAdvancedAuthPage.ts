import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { onBeforeRouteLeave, useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, type AdvancedAuthDetails } from "@/lib/api/config";
import { useConfigStore } from "../../store/config";
import type { AdvancedAuthConfig } from "../../types";
import {
  cloneAdvancedAuthConfig,
  getAdvancedAuthValidationIssue,
  isAdvancedAuthBroadRule,
  snapshotAdvancedAuthConfig,
} from "./advanced-auth-form";
import { normalizeHostLike } from "./model";

export const useSubdomainAdvancedAuthPage = () => {
  const route = useRoute();
  const router = useRouter();
  const { t } = useI18n();
  const configStore = useConfigStore();
  const confirmation = useConfirmationDialog();
  const host = computed(() => String(route.params.host ?? "").trim());
  const loading = ref(true);
  const saving = ref(false);
  const loadError = ref("");
  const missing = ref(false);
  const revision = ref<string | null>(null);
  const savedSnapshot = ref("");
  const confirmedBroadSnapshot = ref("");
  const valueDrafts = reactive<Record<string, string>>({});
  const form = reactive<AdvancedAuthConfig>({
    enabled: false,
    idle_ttl_seconds: 24 * 60 * 60,
    max_lifetime_seconds: 30 * 24 * 60 * 60,
    groups: [],
  });
  const snapshotConfig = () => snapshotAdvancedAuthConfig(form);
  const isDirty = computed(() => snapshotConfig() !== savedSnapshot.value);
  const isBroadRule = computed(() => isAdvancedAuthBroadRule(form));

  const applyDetails = (details: AdvancedAuthDetails) => {
    Object.keys(valueDrafts).forEach((key) => delete valueDrafts[key]);
    revision.value = details.revision;
    const next = cloneAdvancedAuthConfig(details.advanced_auth);
    form.enabled = next.enabled;
    form.idle_ttl_seconds = next.idle_ttl_seconds;
    form.max_lifetime_seconds = next.max_lifetime_seconds;
    form.policy_version = next.policy_version;
    form.groups.splice(0, form.groups.length, ...next.groups);
    savedSnapshot.value = snapshotConfig();
    confirmedBroadSnapshot.value = "";
  };
  const load = async () => {
    loading.value = true;
    loadError.value = "";
    missing.value = false;
    try {
      if (!host.value) throw new Error("Missing host");
      if (!configStore.config) await configStore.loadConfig();
      const mapping = configStore.config?.host_mappings?.find(
        (item) =>
          normalizeHostLike(item.host) === normalizeHostLike(host.value),
      );
      if (
        !mapping ||
        mapping.service_role === "auth" ||
        mapping.use_auth !== true
      ) {
        missing.value = true;
        loadError.value = t("admin.advancedAuth.notFound");
        return;
      }
      applyDetails(await ConfigAPI.getAdvancedAuth(host.value));
    } catch (error) {
      loadError.value = extractErrorMessage(
        error,
        t("admin.advancedAuth.loadFailed"),
      );
      missing.value = true;
    } finally {
      loading.value = false;
    }
  };
  const cancel = () => void router.push("/subdomains");
  const showValidationError = (
    issue: NonNullable<ReturnType<typeof getAdvancedAuthValidationIssue>>,
  ) => {
    if (
      issue.kind === "invalid-source-address" ||
      issue.kind === "invalid-source-cidr"
    ) {
      toast.error(
        t(
          issue.kind === "invalid-source-address"
            ? "admin.advancedAuth.invalidSourceIpLine"
            : "admin.advancedAuth.invalidSourceCidrLine",
          { line: issue.line },
        ),
      );
      return;
    }
    const translationKey = {
      "invalid-rules": "admin.advancedAuth.invalidRules",
      "empty-group": "admin.advancedAuth.emptyGroup",
      "invalid-condition": "admin.advancedAuth.invalidCondition",
      "max-lifetime-too-short": "admin.advancedAuth.maxLifetimeTooShort",
    }[issue.kind];
    toast.error(t(translationKey));
  };
  const save = async () => {
    if (saving.value || !isDirty.value) return;
    const validationIssue = getAdvancedAuthValidationIssue(form);
    if (validationIssue) {
      showValidationError(validationIssue);
      return;
    }
    const pendingSnapshot = snapshotConfig();
    if (
      form.enabled &&
      isBroadRule.value &&
      confirmedBroadSnapshot.value !== pendingSnapshot
    ) {
      const confirmed = await confirmation.requestConfirmation({
        confirmText: t("common.save"),
        description: t("admin.advancedAuth.broadRuleConfirm"),
        title: t("common.confirm"),
      });
      if (!confirmed) return;
      confirmedBroadSnapshot.value = pendingSnapshot;
    }
    const acknowledgeBroadRules =
      form.enabled &&
      isBroadRule.value &&
      confirmedBroadSnapshot.value === pendingSnapshot;
    saving.value = true;
    try {
      const details = await ConfigAPI.updateAdvancedAuth(
        host.value,
        revision.value,
        cloneAdvancedAuthConfig(form),
        acknowledgeBroadRules,
      );
      applyDetails(details);
      void configStore.loadConfig();
      toast.success(t("admin.advancedAuth.saved"));
    } catch (error) {
      toast.error(t("admin.advancedAuth.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.advancedAuth.saveFailedDescription"),
        ),
      });
    } finally {
      saving.value = false;
    }
  };
  const confirmDiscard = () => {
    if (!isDirty.value || saving.value) return true;
    return confirmation.requestConfirmation({
      confirmVariant: "destructive",
      description: t("admin.advancedAuth.discardConfirm"),
      title: t("common.confirm"),
    });
  };
  onBeforeRouteLeave(confirmDiscard);
  const handleBeforeUnload = (event: BeforeUnloadEvent) => {
    if (!isDirty.value) return;
    event.preventDefault();
    event.returnValue = "";
  };
  onMounted(() => {
    void load();
    window.addEventListener("beforeunload", handleBeforeUnload);
  });
  onUnmounted(() =>
    window.removeEventListener("beforeunload", handleBeforeUnload),
  );

  return reactive({
    cancel,
    confirmationDialogOpen: confirmation.confirmationDialogOpen,
    confirmationDialogOptions: confirmation.confirmationDialogOptions,
    confirmPendingAction: confirmation.confirmPendingAction,
    form,
    handleConfirmationDialogOpenChange:
      confirmation.handleConfirmationDialogOpenChange,
    host,
    isBroadRule,
    isDirty,
    loadError,
    loading,
    missing,
    save,
    saving,
    valueDrafts,
  });
};

export type SubdomainAdvancedAuthPageModel = ReturnType<
  typeof useSubdomainAdvancedAuthPage
>;
