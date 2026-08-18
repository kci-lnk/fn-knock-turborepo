import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ScannerAPI, type ScannerPathWhitelist } from "@/lib/api/security";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import {
  normalizeScannerWhitelistPath,
  validateScannerWhitelistEntries,
  type ScannerPathValidationError,
} from "./scannerPathWhitelistModel";

export type ScannerPathWhitelistEntry = {
  id: number;
  value: string;
};

export function useScannerPathWhitelistSettings() {
  const { t } = useI18n();
  const settings = ref<ScannerPathWhitelist | null>(null);
  const entries = ref<ScannerPathWhitelistEntry[]>([]);
  const loadError = ref("");
  let nextEntryId = 0;

  const toEntries = (paths: string[]) =>
    paths.map((value) => ({ id: ++nextEntryId, value }));
  const applySettings = (value: ScannerPathWhitelist) => {
    settings.value = value;
    entries.value = toEntries(value.paths);
  };

  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      loadError.value = extractErrorMessage(
        error,
        t("admin.scannerPathWhitelist.loadFailedDescription"),
      );
      toast.error(t("admin.scannerPathWhitelist.loadFailed"), {
        description: loadError.value,
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSave } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.scannerPathWhitelist.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.scannerPathWhitelist.saveFailedDescription"),
        ),
      });
    },
  });

  const entryErrors = computed<Record<number, string>>(() => {
    const messageKeys: Record<ScannerPathValidationError, string> = {
      required: "admin.scannerPathWhitelist.pathRequired",
      absolute: "admin.scannerPathWhitelist.pathMustBeAbsolute",
      controlCharacters:
        "admin.scannerPathWhitelist.pathContainsControlCharacters",
      duplicate: "admin.scannerPathWhitelist.duplicatePath",
    };
    const errors: Record<number, string> = {};
    for (const [id, error] of validateScannerWhitelistEntries(entries.value)) {
      errors[id] = t(messageKeys[error]);
    }
    return errors;
  });
  const normalizedPaths = computed(() =>
    entries.value.map((entry) => normalizeScannerWhitelistPath(entry.value)),
  );
  const hasSettings = computed(() => settings.value !== null);
  const isDirty = computed(() => {
    if (!settings.value) return false;
    return (
      JSON.stringify(entries.value.map((entry) => entry.value)) !==
      JSON.stringify(settings.value.paths)
    );
  });
  const isDefault = computed(() => {
    if (!settings.value || Object.keys(entryErrors.value).length > 0) {
      return false;
    }
    return (
      JSON.stringify(normalizedPaths.value) ===
      JSON.stringify(settings.value.defaultPaths)
    );
  });

  const fetchSettings = async () => {
    loadError.value = "";
    await runLoad(async () =>
      applySettings(await ScannerAPI.getPathWhitelist()),
    );
  };
  const addEntry = () => {
    entries.value.push({ id: ++nextEntryId, value: "" });
  };
  const setEntryPath = (id: number, value: string) => {
    const entry = entries.value.find((item) => item.id === id);
    if (entry) entry.value = value;
  };
  const removeEntry = (id: number) => {
    entries.value = entries.value.filter((entry) => entry.id !== id);
  };
  const restoreDefaults = () => {
    if (settings.value) {
      entries.value = toEntries(settings.value.defaultPaths);
    }
  };
  const discardChanges = () => {
    if (settings.value) entries.value = toEntries(settings.value.paths);
  };
  const saveSettings = async () => {
    if (Object.keys(entryErrors.value).length > 0) {
      toast.error(t("admin.scannerPathWhitelist.validationFailed"), {
        description: t(
          "admin.scannerPathWhitelist.validationFailedDescription",
        ),
      });
      return;
    }
    await runSave(
      () => ScannerAPI.updatePathWhitelist({ paths: normalizedPaths.value }),
      {
        onSuccess: (value) => {
          applySettings(value);
          toast.success(t("admin.scannerPathWhitelist.saved"));
        },
      },
    );
  };

  onMounted(() => void fetchSettings());

  return reactive({
    addEntry,
    discardChanges,
    entries,
    entryErrors,
    fetchSettings,
    hasSettings,
    isDefault,
    isDirty,
    isLoading,
    isSaving,
    loadError,
    removeEntry,
    restoreDefaults,
    saveSettings,
    setEntryPath,
    showLoadingSkeleton,
  });
}

export type ScannerPathWhitelistSettingsModel = ReturnType<
  typeof useScannerPathWhitelistSettings
>;
