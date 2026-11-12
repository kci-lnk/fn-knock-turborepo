import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "@/store/config";
import type { StreamMapping } from "@/types";
import { sameStreamRule, getMappingKey } from "@/lib/stream-mappings";
import {
  applyStreamRuleAction,
  type StreamRuleAction,
  updateStreamMappingComment,
} from "./streamMappingModel";

export function useStreamMappingMutations() {
  const { t } = useI18n();
  const configStore = useConfigStore();

  function saveWithExpectedRules(
    expected: readonly StreamMapping[],
    update: (current: readonly StreamMapping[]) => StreamMapping[],
    options?: { disableFeatureOnLegacyRepairConflict?: boolean },
  ) {
    return configStore.saveStreamMappings((current) => {
      const byKey = new Map(current.map((rule) => [getMappingKey(rule), rule]));
      for (const rule of expected) {
        const latest = byKey.get(getMappingKey(rule));
        if (!latest || !sameStreamRule(latest, rule))
          throw new Error(t("admin.streamMappings.revisionConflict"));
      }
      return update(current);
    }, options);
  }

  async function saveRuleAction(
    action: StreamRuleAction,
    rules: readonly StreamMapping[],
  ): Promise<boolean> {
    try {
      const keys = new Set(rules.map(getMappingKey));
      const result = await saveWithExpectedRules(
        rules,
        (current) => applyStreamRuleAction(current, keys, action),
        { disableFeatureOnLegacyRepairConflict: action === "delete" },
      );
      toast.success(
        t(`admin.streamMappings.actions.${action}.success`, {
          count: keys.size,
        }),
        {
          description: result.protocolMappingDisabled
            ? t("admin.streamMappings.disabledForLegacyRepair")
            : undefined,
        },
      );
      return true;
    } catch (error) {
      const title =
        action === "delete"
          ? configStore.config?.protocol_mapping_feature?.enabled
            ? "admin.streamMappings.deleteFailed"
            : "admin.streamMappings.deleteFailedWhileDisabled"
          : "admin.streamMappings.saveFailed";
      toast.error(t(title), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
      return false;
    }
  }

  async function updateComment(mapping: StreamMapping, comment: string) {
    try {
      const key = getMappingKey(mapping);
      await saveWithExpectedRules([mapping], (current) =>
        updateStreamMappingComment(current, key, comment),
      );
      toast.success(t("admin.streamMappings.commentUpdated"));
    } catch (error: any) {
      throw new Error(
        extractErrorMessage(
          error,
          t("admin.streamMappings.commentUpdateFailed"),
        ),
        { cause: error },
      );
    }
  }
  return { saveWithExpectedRules, saveRuleAction, updateComment };
}
