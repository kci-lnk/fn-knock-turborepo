import { computed, reactive, ref, watch } from "vue";
import {
  TerminalAPI,
  type TerminalAuthMethod,
  type TerminalCredentialMutation,
  type TerminalErrorCode,
  type TerminalPassphraseMutation,
  type TerminalTargetCreateInput,
  type TerminalTargetRecord,
  type TerminalTargetUpdateInput,
  type TerminalTrustedHostKey,
} from "@/lib/api/terminal";
import { extractTerminalError } from "./terminal-errors";

export interface TerminalTargetDraft {
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: TerminalAuthMethod;
  secret: string;
  passphrase: string;
  clearCredential: boolean;
  clearPassphrase: boolean;
  trustedHostKey: TerminalTrustedHostKey | null;
}

const emptyDraft = (): TerminalTargetDraft => ({
  name: "",
  host: "",
  port: 22,
  username: "",
  authMethod: "password",
  secret: "",
  passphrase: "",
  clearCredential: false,
  clearPassphrase: false,
  trustedHostKey: null,
});

const copyDraft = (draft: TerminalTargetDraft, source: TerminalTargetDraft) =>
  Object.assign(draft, source);

export const useTerminalTargetEditor = ({
  cancelPendingSave,
  createTarget,
  updateTarget,
}: {
  cancelPendingSave?: () => void;
  createTarget: (
    payload: TerminalTargetCreateInput,
  ) => Promise<TerminalTargetRecord>;
  updateTarget: (
    targetId: string,
    payload: TerminalTargetUpdateInput,
    force?: boolean,
    confirmationToken?: string,
  ) => Promise<TerminalTargetRecord>;
}) => {
  const open = ref(false);
  const editingTarget = ref<TerminalTargetRecord | null>(null);
  const draft = reactive<TerminalTargetDraft>(emptyDraft());
  const pendingHostKey = ref<TerminalTrustedHostKey | null>(null);
  const testing = ref(false);
  const saving = ref(false);
  const testedGeneration = ref<number | null>(null);
  const verificationToken = ref("");
  const error = ref("");
  const errorCode = ref<TerminalErrorCode | null>(null);
  const conflictingActiveSessionCount = ref<number | null>(null);
  const forceConfirmationRequired = ref(false);
  const forceConfirmationToken = ref("");
  let connectionGeneration = 0;
  let operationGeneration = 0;
  let operationController: AbortController | null = null;
  let suppressConnectionInvalidation = false;

  const clearSensitiveDraft = () => {
    suppressConnectionInvalidation = true;
    draft.secret = "";
    draft.passphrase = "";
    pendingHostKey.value = null;
    verificationToken.value = "";
    suppressConnectionInvalidation = false;
  };

  watch(
    [
      () => draft.host,
      () => draft.port,
      () => draft.username,
      () => draft.authMethod,
      () => draft.secret,
      () => draft.passphrase,
      () => draft.clearCredential,
      () => draft.clearPassphrase,
      () => draft.trustedHostKey?.algorithm,
      () => draft.trustedHostKey?.fingerprint,
    ],
    () => {
      if (suppressConnectionInvalidation) return;
      connectionGeneration += 1;
      testedGeneration.value = null;
      verificationToken.value = "";
      operationGeneration += 1;
      operationController?.abort();
      operationController = null;
      cancelPendingSave?.();
      testing.value = false;
      error.value = "";
      errorCode.value = null;
      conflictingActiveSessionCount.value = null;
      forceConfirmationRequired.value = false;
      forceConfirmationToken.value = "";
    },
    { flush: "sync" },
  );

  const endpointKey = computed(() => `${draft.host.trim()}:${draft.port}`);
  const credentialConfigured = computed(
    () =>
      editingTarget.value?.credentialConfigured === true &&
      editingTarget.value.authMethod === draft.authMethod,
  );
  const passphraseConfigured = computed(
    () =>
      editingTarget.value?.passphraseConfigured === true &&
      draft.authMethod === "privateKey" &&
      editingTarget.value.authMethod === draft.authMethod,
  );
  const hasCredential = computed(() => {
    if (draft.clearCredential) return false;
    return Boolean(draft.secret || credentialConfigured.value);
  });
  const testable = computed(() =>
    Boolean(
      draft.host.trim() &&
      draft.username.trim() &&
      Number.isInteger(draft.port) &&
      draft.port >= 1 &&
      draft.port <= 65535 &&
      hasCredential.value,
    ),
  );
  const requiresSessionTermination = computed(() => {
    const saved = editingTarget.value;
    if (!saved) return false;
    return (
      draft.host.trim() !== saved.host ||
      draft.port !== saved.port ||
      draft.username.trim() !== saved.username ||
      draft.authMethod !== saved.authMethod ||
      draft.trustedHostKey?.algorithm !== saved.trustedHostKey?.algorithm ||
      draft.trustedHostKey?.fingerprint !== saved.trustedHostKey?.fingerprint ||
      credentialMutation().action !== "keep" ||
      passphraseMutation().action !== "keep"
    );
  });
  const tested = computed(
    () =>
      Boolean(verificationToken.value) &&
      testedGeneration.value === connectionGeneration,
  );
  const valid = computed(() =>
    Boolean(
      draft.name.trim() &&
      draft.host.trim() &&
      draft.username.trim() &&
      Number.isInteger(draft.port) &&
      draft.port >= 1 &&
      draft.port <= 65535 &&
      draft.trustedHostKey &&
      (hasCredential.value || draft.clearCredential),
    ),
  );
  const canSave = computed(() => {
    if (
      editingTarget.value &&
      !requiresSessionTermination.value &&
      draft.name.trim()
    ) {
      return true;
    }
    return valid.value && (draft.clearCredential || tested.value);
  });

  const resetAsyncState = () => {
    operationGeneration += 1;
    operationController?.abort();
    operationController = null;
    cancelPendingSave?.();
    testing.value = false;
    saving.value = false;
    error.value = "";
    errorCode.value = null;
    conflictingActiveSessionCount.value = null;
    forceConfirmationRequired.value = false;
    forceConfirmationToken.value = "";
  };

  const beginCreate = () => {
    resetAsyncState();
    editingTarget.value = null;
    copyDraft(draft, emptyDraft());
    pendingHostKey.value = null;
    testedGeneration.value = null;
    verificationToken.value = "";
    open.value = true;
  };

  const beginEdit = (target: TerminalTargetRecord) => {
    resetAsyncState();
    editingTarget.value = target;
    copyDraft(draft, {
      name: target.name,
      host: target.host,
      port: target.port,
      username: target.username,
      authMethod: target.authMethod,
      secret: "",
      passphrase: "",
      clearCredential: false,
      clearPassphrase: false,
      trustedHostKey: target.trustedHostKey
        ? { ...target.trustedHostKey }
        : null,
    });
    pendingHostKey.value = null;
    testedGeneration.value = null;
    verificationToken.value = "";
    open.value = true;
  };

  const invalidateTrust = () => {
    draft.trustedHostKey = null;
    pendingHostKey.value = null;
  };

  const setEndpoint = (field: "host" | "port", value: string | number) => {
    if (field === "host") draft.host = String(value);
    else draft.port = Number(value);
    const saved = editingTarget.value;
    if (
      !saved ||
      draft.host.trim() !== saved.host ||
      draft.port !== saved.port
    ) {
      invalidateTrust();
    } else {
      draft.trustedHostKey = saved.trustedHostKey
        ? { ...saved.trustedHostKey }
        : null;
    }
  };

  const setAuthMethod = (authMethod: TerminalAuthMethod) => {
    if (draft.authMethod === authMethod) return;
    draft.authMethod = authMethod;
    draft.secret = "";
    draft.passphrase = "";
    draft.clearCredential = false;
    draft.clearPassphrase = false;
  };

  const beginOperation = () => {
    const generation = ++operationGeneration;
    operationController?.abort();
    operationController = new AbortController();
    error.value = "";
    errorCode.value = null;
    conflictingActiveSessionCount.value = null;
    forceConfirmationToken.value = "";
    return { generation, signal: operationController.signal };
  };

  const credentialMutation = (): TerminalCredentialMutation => {
    if (draft.clearCredential) return { action: "clear" };
    if (!draft.secret) return { action: "keep" };
    return { action: "replace", secret: draft.secret };
  };

  const passphraseMutation = (): TerminalPassphraseMutation => {
    if (draft.authMethod !== "privateKey") return { action: "keep" };
    if (draft.clearCredential || draft.clearPassphrase) {
      return { action: "clear" };
    }
    if (!draft.passphrase) return { action: "keep" };
    return { action: "replace", secret: draft.passphrase };
  };

  const buildPayloadFields = () => ({
    name: draft.name.trim(),
    host: draft.host.trim(),
    port: draft.port,
    username: draft.username.trim(),
    authMethod: draft.authMethod,
    trustedHostKey: draft.trustedHostKey ? { ...draft.trustedHostKey } : null,
    credential: credentialMutation(),
    passphrase: passphraseMutation(),
    ...(verificationToken.value
      ? { verificationToken: verificationToken.value }
      : {}),
  });

  const buildCreatePayload = (): TerminalTargetCreateInput =>
    buildPayloadFields();

  const buildUpdatePayload = (
    target: TerminalTargetRecord,
  ): TerminalTargetUpdateInput => ({
    ...buildPayloadFields(),
    revision: target.revision,
  });

  const sameHostKey = (
    left: TerminalTrustedHostKey | null,
    right: TerminalTrustedHostKey,
  ) =>
    left?.algorithm === right.algorithm &&
    left.fingerprint === right.fingerprint;

  const runConnectionTest = async (skipProbe: boolean) => {
    if (!testable.value) return false;
    const testedConnectionGeneration = connectionGeneration;
    const endpoint = endpointKey.value;
    const operation = beginOperation();
    testing.value = true;
    pendingHostKey.value = null;
    try {
      let trustedHostKey = draft.trustedHostKey;
      if (!skipProbe) {
        const result = await TerminalAPI.probeHostKey(
          { host: draft.host.trim(), port: draft.port },
          operation.signal,
        );
        if (
          operation.generation !== operationGeneration ||
          testedConnectionGeneration !== connectionGeneration ||
          endpoint !== endpointKey.value
        ) {
          return false;
        }
        const liveHostKey = {
          algorithm: result.algorithm,
          fingerprint: result.fingerprint,
        };
        if (!sameHostKey(trustedHostKey, liveHostKey)) {
          pendingHostKey.value = liveHostKey;
          return false;
        }
        trustedHostKey = liveHostKey;
      }
      if (!trustedHostKey) return false;
      const result = await TerminalAPI.testConnection(
        {
          ...(editingTarget.value ? { targetId: editingTarget.value.id } : {}),
          draft: {
            host: draft.host.trim(),
            port: draft.port,
            username: draft.username.trim(),
            authMethod: draft.authMethod,
            trustedHostKey: { ...trustedHostKey },
          },
          credential: credentialMutation(),
          passphrase: passphraseMutation(),
        },
        operation.signal,
      );
      if (
        operation.generation !== operationGeneration ||
        testedConnectionGeneration !== connectionGeneration
      ) {
        return false;
      }
      testedGeneration.value = testedConnectionGeneration;
      verificationToken.value = result.verificationToken;
      return true;
    } catch (reason) {
      if (!operation.signal.aborted) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
      }
      return false;
    } finally {
      if (operation.generation === operationGeneration) testing.value = false;
    }
  };

  const testConnection = () => runConnectionTest(false);

  const confirmHostKey = async () => {
    const hostKey = pendingHostKey.value;
    if (!hostKey) return false;
    pendingHostKey.value = null;
    draft.trustedHostKey = { ...hostKey };
    return runConnectionTest(true);
  };

  const save = async (force = false) => {
    if (!canSave.value) return null;
    const generation = ++operationGeneration;
    operationController?.abort();
    operationController = null;
    saving.value = true;
    error.value = "";
    errorCode.value = null;
    const confirmedForce = Boolean(
      force &&
      forceConfirmationRequired.value &&
      forceConfirmationToken.value &&
      requiresSessionTermination.value,
    );
    try {
      const currentTarget = editingTarget.value;
      const target = currentTarget
        ? await updateTarget(
            currentTarget.id,
            buildUpdatePayload(currentTarget),
            confirmedForce,
            confirmedForce ? forceConfirmationToken.value : undefined,
          )
        : await createTarget(buildCreatePayload());
      if (generation !== operationGeneration) return null;
      clearSensitiveDraft();
      conflictingActiveSessionCount.value = null;
      forceConfirmationRequired.value = false;
      forceConfirmationToken.value = "";
      open.value = false;
      return target;
    } catch (reason) {
      if (generation === operationGeneration) {
        const failure = extractTerminalError(reason);
        error.value = failure.message;
        errorCode.value = failure.errorCode;
        conflictingActiveSessionCount.value = failure.activeSessionCount;
        if (
          failure.errorCode === "conflict" &&
          requiresSessionTermination.value &&
          failure.confirmationToken
        ) {
          forceConfirmationRequired.value = true;
          forceConfirmationToken.value = failure.confirmationToken;
        } else {
          forceConfirmationRequired.value = false;
          forceConfirmationToken.value = "";
        }
      }
      return null;
    } finally {
      if (generation === operationGeneration) saving.value = false;
    }
  };

  const close = () => {
    resetAsyncState();
    clearSensitiveDraft();
    open.value = false;
  };

  return {
    beginCreate,
    beginEdit,
    canSave,
    close,
    confirmHostKey,
    conflictingActiveSessionCount,
    credentialConfigured,
    draft,
    editingTarget,
    error,
    errorCode,
    forceConfirmationRequired,
    forceConfirmationToken,
    open,
    pendingHostKey,
    passphraseConfigured,
    requiresSessionTermination,
    save,
    setAuthMethod,
    setEndpoint,
    tested,
    testable,
    testing,
    saving,
    testConnection,
    valid,
  };
};
