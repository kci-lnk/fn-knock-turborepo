import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import {
  onBeforeRouteLeave,
  onBeforeRouteUpdate,
  useRoute,
  useRouter,
} from "vue-router";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, type StreamBypassPolicy } from "@/lib/api/config";
import { useConfigStore } from "../../store/config";
import type { StreamMapping } from "../../types";
import {
  cloneStreamBypassPolicy,
  createBlankStreamBypassGroup,
  getStreamBypassValidationIssue,
  isBroadStreamBypassPolicy,
  snapshotStreamBypassPolicy,
  toStreamBypassPolicyPayload,
  type StreamBypassPolicyForm,
} from "./stream-bypass-policy-form";

const emptyPolicy = (): StreamBypassPolicy => ({
  broad_rule_confirmed: false,
  enabled: false,
  groups: [],
  policy_version: "",
});

export const useStreamBypassPolicyPage = () => {
  const route = useRoute();
  const router = useRouter();
  const { t } = useI18n();
  const configStore = useConfigStore();
  const confirmation = useConfirmationDialog();
  const protocol = computed(() =>
    String(route.params.protocol ?? "")
      .trim()
      .toLowerCase(),
  );
  const listenPort = computed(() => Number(route.params.port));
  const loading = ref(true);
  const saving = ref(false);
  const missing = ref(false);
  const loadError = ref("");
  const mapping = ref<StreamMapping | null>(null);
  const savedSnapshot = ref("");
  const confirmedBroadSnapshot = ref("");
  const valueDrafts = reactive<Record<string, string>>({});
  const form = reactive<StreamBypassPolicyForm>(
    cloneStreamBypassPolicy(emptyPolicy()),
  );
  let loadSequence = 0;
  const snapshot = () => snapshotStreamBypassPolicy(form);
  const isDirty = computed(() => snapshot() !== savedSnapshot.value);
  const isBroadRule = computed(() => isBroadStreamBypassPolicy(form));
  const authEnabled = computed(() => mapping.value?.use_auth === true);
  const mappingLabel = computed(() => {
    const current = mapping.value;
    return current
      ? `${current.protocol.toUpperCase()} :${current.listen_port} → ${current.target}`
      : `${protocol.value.toUpperCase()} :${listenPort.value}`;
  });

  const applyPolicy = (policy: StreamBypassPolicy) => {
    Object.keys(valueDrafts).forEach((key) => delete valueDrafts[key]);
    const next = cloneStreamBypassPolicy(policy);
    form.enabled = next.enabled;
    form.policy_version = next.policy_version;
    form.broad_rule_confirmed = false;
    form.groups.splice(0, form.groups.length, ...next.groups);
    savedSnapshot.value = snapshot();
    confirmedBroadSnapshot.value = "";
  };

  const findMapping = (
    requestedProtocol = protocol.value,
    requestedPort = listenPort.value,
  ) =>
    configStore.config?.stream_mappings?.find(
      (candidate) =>
        candidate.protocol === requestedProtocol &&
        candidate.listen_port === requestedPort,
    ) ?? null;

  const load = async () => {
    const request = ++loadSequence;
    const requestedProtocol = protocol.value;
    const requestedPort = listenPort.value;
    loading.value = true;
    loadError.value = "";
    missing.value = false;
    mapping.value = null;
    try {
      if (
        !["tcp", "udp"].includes(requestedProtocol) ||
        !Number.isInteger(requestedPort) ||
        requestedPort < 1 ||
        requestedPort > 65_535
      ) {
        throw new Error(t("admin.streamMappings.policyNotFound"));
      }
      // Layout removes the active RouterView while the shared config store is
      // loading. Forcing a reload from this page's mount path would therefore
      // unmount the page, then mount it again as soon as the request completes,
      // creating an endless config-request loop. Layout has normally loaded the
      // config before rendering this route; only fill it in when it is absent.
      if (!configStore.config) {
        await configStore.loadConfig();
      }
      if (request !== loadSequence) return;
      const requestedMapping = findMapping(requestedProtocol, requestedPort);
      if (!requestedMapping) {
        missing.value = true;
        loadError.value = t("admin.streamMappings.policyNotFound");
        return;
      }
      const policy = await ConfigAPI.getStreamBypassPolicy(requestedMapping);
      if (request !== loadSequence) return;
      mapping.value = requestedMapping;
      applyPolicy(policy);
    } catch (error) {
      if (request !== loadSequence) return;
      missing.value = true;
      loadError.value = extractErrorMessage(
        error,
        t("admin.streamMappings.policyLoadFailed"),
      );
    } finally {
      if (request === loadSequence) loading.value = false;
    }
  };

  const cancel = () => void router.push("/streams");

  const setEnabled = (enabled: boolean) => {
    if (enabled && !authEnabled.value) return;
    form.enabled = enabled;
    if (enabled && form.groups.length === 0) {
      form.groups.push(createBlankStreamBypassGroup());
    }
  };

  const showValidationError = (
    issue: NonNullable<ReturnType<typeof getStreamBypassValidationIssue>>,
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
    const key = {
      "empty-group": "admin.streamMappings.policyEmptyGroup",
      "invalid-condition": "admin.streamMappings.policyInvalidCondition",
      "missing-rules": "admin.streamMappings.policyMissingRules",
    }[issue.kind];
    toast.error(t(key));
  };

  const save = async () => {
    const current = mapping.value;
    if (!current || saving.value || !isDirty.value) return;
    const issue = getStreamBypassValidationIssue(form);
    if (issue) {
      showValidationError(issue);
      return;
    }
    const pendingSnapshot = snapshot();
    if (
      form.enabled &&
      isBroadRule.value &&
      confirmedBroadSnapshot.value !== pendingSnapshot
    ) {
      const confirmed = await confirmation.requestConfirmation({
        confirmText: t("common.save"),
        description: t("admin.streamMappings.policyBroadRuleConfirm"),
        title: t("common.confirm"),
      });
      if (!confirmed) return;
      confirmedBroadSnapshot.value = pendingSnapshot;
    }
    const broadConfirmed =
      form.enabled &&
      isBroadRule.value &&
      confirmedBroadSnapshot.value === pendingSnapshot;
    saving.value = true;
    try {
      const saved = await ConfigAPI.updateStreamBypassPolicy(
        current,
        toStreamBypassPolicyPayload(form, broadConfirmed),
      );
      applyPolicy(saved);
      await configStore.refreshStreamMappingsOnly();
      mapping.value = findMapping();
      toast.success(t("admin.streamMappings.policySaved"));
    } catch (error) {
      toast.error(t("admin.streamMappings.policySaveFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    } finally {
      saving.value = false;
    }
  };

  const confirmDiscard = () => {
    if (saving.value) return false;
    if (!isDirty.value) return true;
    return confirmation.requestConfirmation({
      confirmVariant: "destructive",
      description: t("admin.streamMappings.policyDiscardConfirm"),
      title: t("common.confirm"),
    });
  };
  onBeforeRouteLeave(confirmDiscard);
  onBeforeRouteUpdate(confirmDiscard);
  const handleBeforeUnload = (event: BeforeUnloadEvent) => {
    if (!isDirty.value) return;
    event.preventDefault();
    event.returnValue = "";
  };
  onMounted(() => {
    window.addEventListener("beforeunload", handleBeforeUnload);
  });
  watch([protocol, listenPort], () => void load(), { immediate: true });
  onUnmounted(() => {
    loadSequence += 1;
    window.removeEventListener("beforeunload", handleBeforeUnload);
  });

  return reactive({
    authEnabled,
    cancel,
    confirmationDialogOpen: confirmation.confirmationDialogOpen,
    confirmationDialogOptions: confirmation.confirmationDialogOptions,
    confirmPendingAction: confirmation.confirmPendingAction,
    form,
    handleConfirmationDialogOpenChange:
      confirmation.handleConfirmationDialogOpenChange,
    isBroadRule,
    isDirty,
    listenPort,
    loadError,
    loading,
    mapping,
    mappingLabel,
    missing,
    protocol,
    save,
    saving,
    setEnabled,
    valueDrafts,
  });
};

export type StreamBypassPolicyPageModel = ReturnType<
  typeof useStreamBypassPolicyPage
>;
