import { computed, ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type {
  AuthAccount,
  AuthLoginMode,
  TOTPCredential,
  TOTPCredentialImportSummary,
} from "../../types";

const MAX_IMPORT_FILE_SIZE = 512 * 1024;

type Translate = (key: string, params?: Record<string, unknown>) => string;

interface UseAuthCredentialTransferOptions {
  authAccounts: Ref<AuthAccount[]>;
  authLoginMode: Ref<AuthLoginMode>;
  credentials: Ref<TOTPCredential[]>;
  refreshStatus: () => Promise<unknown>;
  translate: Translate;
}

export function useAuthCredentialTransfer({
  authAccounts,
  authLoginMode,
  credentials,
  refreshStatus,
  translate,
}: UseAuthCredentialTransferOptions) {
  const credentialImportInputRef = ref<HTMLInputElement | null>(null);
  const showCredentialTransferDialog = ref(false);
  const showExportDialog = ref(false);
  const showImportDialog = ref(false);
  const pendingCredentialImportPayload = ref<unknown>(null);
  const pendingCredentialImportFilename = ref("");

  const { isPending: isExportingCredentials, run: runExportCredentials } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            translate("admin.authSettings.exportCredentialsFailed"),
          ),
        );
      },
    });
  const { isPending: isImportingCredentials, run: runImportCredentials } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            translate("admin.authSettings.importCredentialsFailed"),
          ),
        );
      },
    });

  const isCredentialTransferBusy = computed(
    () => isExportingCredentials.value || isImportingCredentials.value,
  );
  const exportableCredentialCount = computed(() =>
    authLoginMode.value === "password"
      ? authAccounts.value.length
      : credentials.value.length,
  );

  function buildCredentialExportFilename() {
    const prefix =
      authLoginMode.value === "password"
        ? "fn-knock-password-credentials"
        : "fn-knock-totp-credentials";
    return `${prefix}-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  }

  function resetPendingCredentialImport() {
    pendingCredentialImportPayload.value = null;
    pendingCredentialImportFilename.value = "";
  }

  function resetCredentialImportInput() {
    if (credentialImportInputRef.value) {
      credentialImportInputRef.value.value = "";
    }
  }

  function setCredentialImportInput(element: unknown) {
    credentialImportInputRef.value =
      element instanceof HTMLInputElement ? element : null;
  }

  function openExportDialog() {
    if (exportableCredentialCount.value === 0 || isCredentialTransferBusy.value) {
      return;
    }
    showExportDialog.value = true;
  }

  function openExportDialogFromCredentialTransferDialog() {
    showCredentialTransferDialog.value = false;
    openExportDialog();
  }

  async function handleExportCredentials() {
    await runExportCredentials(async () => {
      const blob = await ConfigAPI.downloadTOTPCredentials();
      downloadBlob(blob, buildCredentialExportFilename());
      showExportDialog.value = false;
      toast.success(translate("admin.authSettings.exportCredentialsStarted"));
    });
  }

  function triggerImportFilePicker() {
    if (isCredentialTransferBusy.value) return;
    resetCredentialImportInput();
    credentialImportInputRef.value?.click();
  }

  function triggerImportFilePickerFromCredentialTransferDialog() {
    showCredentialTransferDialog.value = false;
    triggerImportFilePicker();
  }

  async function handleCredentialImportFileChange(event: Event) {
    const input = event.target as HTMLInputElement | null;
    const file = input?.files?.[0] ?? null;
    resetPendingCredentialImport();

    if (!file) return;

    if (
      !file.name.toLowerCase().endsWith(".json") &&
      file.type !== "application/json"
    ) {
      toast.error(translate("admin.authSettings.importCredentialsInvalidFile"));
      resetCredentialImportInput();
      return;
    }

    if (file.size > MAX_IMPORT_FILE_SIZE) {
      toast.error(translate("admin.authSettings.importCredentialsFileTooLarge"), {
        description: translate(
          "admin.authSettings.importCredentialsFileTooLargeDetail",
          { size: Math.floor(MAX_IMPORT_FILE_SIZE / 1024) },
        ),
      });
      resetCredentialImportInput();
      return;
    }

    try {
      pendingCredentialImportPayload.value = JSON.parse(await file.text());
      pendingCredentialImportFilename.value = file.name;
      showImportDialog.value = true;
    } catch {
      toast.error(translate("admin.authSettings.importCredentialsParseFailed"));
    } finally {
      resetCredentialImportInput();
    }
  }

  function buildImportSummaryDescription(summary: TOTPCredentialImportSummary) {
    if (
      summary.kind === "password" ||
      summary.login_mode === "password" ||
      typeof summary.password_total === "number"
    ) {
      return translate("admin.authSettings.importPasswordCredentialsSummary", {
        total: summary.total,
        imported: summary.imported,
        skippedExistingId: summary.skipped_existing_id,
        skippedExistingUsername: summary.skipped_existing_username ?? 0,
        skippedFileDuplicate: summary.skipped_file_duplicate,
        invalid: summary.invalid,
        passwordTotal: summary.password_total ?? 0,
        passwordImported: summary.password_imported ?? 0,
        passwordSkippedExisting: summary.password_skipped_existing ?? 0,
        passwordSkippedMissingAccount:
          summary.password_skipped_missing_account ?? 0,
        passwordSkippedFileDuplicate:
          summary.password_skipped_file_duplicate ?? 0,
        passwordInvalid: summary.password_invalid ?? 0,
        totpTotal: summary.totp_total ?? 0,
        totpImported: summary.totp_imported ?? 0,
      });
    }

    return translate("admin.authSettings.importCredentialsSummary", {
      imported: summary.imported,
      skippedExistingId: summary.skipped_existing_id,
      skippedExistingSecret: summary.skipped_existing_secret ?? 0,
      skippedFileDuplicate: summary.skipped_file_duplicate,
      invalid: summary.invalid,
      total: summary.total,
    });
  }

  async function handleImportCredentials() {
    const payload = pendingCredentialImportPayload.value;
    if (!payload) {
      toast.error(translate("admin.authSettings.importCredentialsChooseFileFirst"));
      return;
    }

    await runImportCredentials(async () => {
      const summary = await ConfigAPI.importTOTPCredentials(payload);
      showImportDialog.value = false;
      resetPendingCredentialImport();
      await refreshStatus();
      toast.success(translate("admin.authSettings.importCredentialsCompleted"), {
        description: buildImportSummaryDescription(summary),
      });
    });
  }

  return {
    exportableCredentialCount,
    handleCredentialImportFileChange,
    handleExportCredentials,
    handleImportCredentials,
    isCredentialTransferBusy,
    isExportingCredentials,
    isImportingCredentials,
    openExportDialogFromCredentialTransferDialog,
    pendingCredentialImportFilename,
    resetPendingCredentialImport,
    setCredentialImportInput,
    showCredentialTransferDialog,
    showExportDialog,
    showImportDialog,
    triggerImportFilePickerFromCredentialTransferDialog,
  };
}
