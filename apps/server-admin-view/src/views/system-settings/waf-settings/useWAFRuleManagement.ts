import { computed, ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { WAFAPI } from "@/lib/api/gateway";
import type {
  WAFDetails,
  WAFRuleFile,
  WAFRuleFileContent,
  WAFRuleSource,
} from "@/types";

const SYSTEM_INITIALIZATION_RULE_FILENAME = "REQUEST-901-INITIALIZATION.conf";

export const useWAFRuleManagement = ({
  applyDetails,
  details,
  formatDate,
  selectedCustomRules,
  selectedSystemRules,
}: {
  applyDetails: (data: WAFDetails) => void;
  details: Ref<WAFDetails | null>;
  formatDate: (value?: string | null) => string;
  selectedCustomRules: Ref<string[]>;
  selectedSystemRules: Ref<string[]>;
}) => {
  const { t } = useI18n();
  const uploadInputRef = ref<HTMLInputElement | null>(null);
  const activeRuleActionsKey = ref("");
  const loadingRuleKey = ref("");
  const downloadingRuleKey = ref("");
  const isRulePreviewOpen = ref(false);
  const activeRulePreview = ref<WAFRuleFileContent | null>(null);

  const { isPending: isUpdatingSystemRules, run: runUpdateSystemRules } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.wafSettings.updateFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.wafSettings.systemUpdateDescription"),
          ),
        });
      },
    });
  const { isPending: isUploading, run: runUploadRules } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.wafSettings.uploadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.uploadDescription"),
        ),
      });
    },
  });
  const { isPending: isChangingRules, run: runRuleChange } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.wafSettings.updateFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.ruleUpdateDescription"),
        ),
      });
    },
  });

  const systemRules = computed(() =>
    (details.value?.system.rules || []).filter(
      (rule) => rule.filename !== SYSTEM_INITIALIZATION_RULE_FILENAME,
    ),
  );
  const customRules = computed(() => details.value?.custom.rules || []);
  const sourceRules = (source: WAFRuleSource) =>
    source === "system" ? systemRules.value : customRules.value;
  const allRulesEnabled = (source: WAFRuleSource) => {
    const rules = sourceRules(source);
    return rules.length > 0 && rules.every((rule) => rule.enabled);
  };

  const formatSize = (value: number) => {
    if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`;
    if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
    return `${value} B`;
  };
  const formatSystemRuleName = (rule: WAFRuleFile) =>
    rule.filename.replace(/\.conf$/i, "");
  const formatSystemRuleMeta = (rule: WAFRuleFile) => rule.description;
  const formatRuleSize = (rule: WAFRuleFile) => formatSize(rule.size_bytes);
  const formatCustomRuleName = (rule: WAFRuleFile) => rule.filename;
  const formatCustomRuleMeta = (rule: WAFRuleFile) =>
    `${formatSize(rule.size_bytes)} · ${formatDate(rule.updated_at)}`;
  const sourceLabel = (source: WAFRuleSource) =>
    source === "system"
      ? t("admin.wafSettings.systemRules")
      : t("admin.wafSettings.customRules");
  const ruleKey = (rule: Pick<WAFRuleFile, "source" | "filename">) =>
    `${rule.source}:${rule.filename}`;
  const activateRuleActions = (
    rule: Pick<WAFRuleFile, "source" | "filename">,
  ) => {
    activeRuleActionsKey.value = ruleKey(rule);
  };

  const refreshAndSyncSystemRules = async () => {
    const refreshed = await WAFAPI.refreshManifest();
    if (
      !refreshed.system.update_available &&
      refreshed.system.rules.length > 0
    ) {
      return { details: refreshed, updated: false };
    }
    return { details: await WAFAPI.syncSystemRules(), updated: true };
  };

  const updateSystemRules = async () => {
    await runUpdateSystemRules(refreshAndSyncSystemRules, {
      onSuccess: ({ details: nextDetails, updated }) => {
        applyDetails(nextDetails);
        if (updated) {
          toast.success(t("admin.wafSettings.systemRulesUpdated"), {
            description: nextDetails.config.enabled
              ? t("admin.wafSettings.loadedToGateway")
              : t("admin.wafSettings.rulesApplyWhenEnabled"),
          });
          return;
        }
        toast.success(t("admin.wafSettings.rulesLatest"), {
          description: t("admin.wafSettings.noNewSystemRules"),
        });
      },
    });
  };

  const selectionRef = (source: WAFRuleSource) =>
    source === "system" ? selectedSystemRules : selectedCustomRules;
  const setRuleSelected = (
    source: WAFRuleSource,
    filename: string,
    checked: boolean,
  ) => {
    const target = selectionRef(source);
    target.value = checked
      ? [...new Set([...target.value, filename])]
      : target.value.filter((item) => item !== filename);
  };
  const setAllSelected = (source: WAFRuleSource, checked: boolean) => {
    selectionRef(source).value = checked
      ? sourceRules(source).map((rule) => rule.filename)
      : [];
  };

  const updateRulesEnabled = async (
    source: WAFRuleSource,
    filenames: string[] | undefined,
    enabled: boolean,
  ) => {
    await runRuleChange(
      () => WAFAPI.setRulesEnabled({ source, filenames, enabled }),
      {
        onSuccess: (data) => {
          applyDetails(data);
          toast.success(
            enabled
              ? t("admin.wafSettings.ruleEnabled")
              : t("admin.wafSettings.ruleDisabled"),
            {
              description: data.config.enabled
                ? t("admin.wafSettings.loadedToGateway")
                : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
            },
          );
        },
      },
    );
  };

  const toggleRule = (rule: WAFRuleFile, enabled: boolean) =>
    updateRulesEnabled(rule.source, [rule.filename], enabled);
  const updateSelectedRules = (source: WAFRuleSource, enabled: boolean) => {
    const filenames = selectionRef(source).value;
    if (filenames.length === 0) return;
    return updateRulesEnabled(source, filenames, enabled);
  };
  const toggleAllRules = (source: WAFRuleSource) =>
    updateRulesEnabled(source, undefined, !allRulesEnabled(source));

  const enableRecommendedSystemRules = async () => {
    await runRuleChange(WAFAPI.enableRecommendedSystemRules, {
      onSuccess: (data) => {
        applyDetails(data);
        toast.success(t("admin.wafSettings.recommendedRulesEnabled"), {
          description: data.config.enabled
            ? t("admin.wafSettings.loadedToGateway")
            : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
        });
      },
    });
  };

  const openRulePreview = async (rule: WAFRuleFile) => {
    const key = ruleKey(rule);
    activateRuleActions(rule);
    loadingRuleKey.value = key;
    try {
      activeRulePreview.value = await WAFAPI.getRuleFile(
        rule.source,
        rule.filename,
      );
      isRulePreviewOpen.value = true;
    } catch (error) {
      toast.error(t("admin.wafSettings.readFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.ruleReadDescription"),
        ),
      });
    } finally {
      if (loadingRuleKey.value === key) loadingRuleKey.value = "";
    }
  };

  const downloadRuleFile = async (rule: WAFRuleFile) => {
    const key = ruleKey(rule);
    activateRuleActions(rule);
    downloadingRuleKey.value = key;
    try {
      const data = await WAFAPI.getRuleFile(rule.source, rule.filename);
      downloadBlob(
        new Blob([data.content], { type: "text/plain;charset=utf-8" }),
        data.filename || rule.filename,
      );
    } catch (error) {
      toast.error(t("admin.wafSettings.downloadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.ruleDownloadDescription"),
        ),
      });
    } finally {
      if (downloadingRuleKey.value === key) downloadingRuleKey.value = "";
    }
  };

  const readFileAsBase64 = (file: File): Promise<string> =>
    new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const value = String(reader.result || "");
        resolve(value.includes(",") ? value.split(",")[1] || "" : value);
      };
      reader.onerror = () =>
        reject(
          reader.error || new Error(t("admin.wafSettings.fileReadFailed")),
        );
      reader.readAsDataURL(file);
    });

  const triggerUpload = () => uploadInputRef.value?.click();
  const handleUploadChange = async (event: Event) => {
    const input = event.target as HTMLInputElement;
    const files = Array.from(input.files || []);
    input.value = "";
    if (files.length === 0) return;
    await runUploadRules(
      async () =>
        WAFAPI.uploadCustomRules({
          files: await Promise.all(
            files.map(async (file) => ({
              filename: file.name,
              content_base64: await readFileAsBase64(file),
            })),
          ),
        }),
      {
        onSuccess: (data) => {
          applyDetails(data);
          toast.success(t("admin.wafSettings.customRulesUploaded"), {
            description: data.config.enabled
              ? t("admin.wafSettings.loadedToGateway")
              : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
          });
        },
      },
    );
  };

  const deleteCustomRule = async (filename: string) => {
    await runRuleChange(() => WAFAPI.deleteCustomRule(filename), {
      onSuccess: (data) => {
        applyDetails(data);
        toast.success(t("admin.wafSettings.customRuleDeleted"), {
          description: data.config.enabled
            ? t("admin.wafSettings.loadedToGateway")
            : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
        });
      },
    });
  };

  return {
    activateRuleActions,
    activeRuleActionsKey,
    activeRulePreview,
    allRulesEnabled,
    customRules,
    deleteCustomRule,
    downloadingRuleKey,
    downloadRuleFile,
    enableRecommendedSystemRules,
    formatCustomRuleMeta,
    formatCustomRuleName,
    formatRuleSize,
    formatSize,
    formatSystemRuleMeta,
    formatSystemRuleName,
    handleUploadChange,
    isChangingRules,
    isRulePreviewOpen,
    isUpdatingSystemRules,
    isUploading,
    loadingRuleKey,
    openRulePreview,
    refreshAndSyncSystemRules,
    ruleKey,
    setAllSelected,
    setRuleSelected,
    sourceLabel,
    systemRules,
    toggleAllRules,
    toggleRule,
    triggerUpload,
    updateSelectedRules,
    updateSystemRules,
    uploadInputRef,
  };
};
