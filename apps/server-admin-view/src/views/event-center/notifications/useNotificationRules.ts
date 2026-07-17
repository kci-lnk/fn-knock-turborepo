import type { MaybeRefOrGetter } from "vue";
import { useNotificationRuleEditor } from "./useNotificationRuleEditor";
import { useNotificationRulesResource } from "./useNotificationRulesResource";

export function useNotificationRules(active: MaybeRefOrGetter<boolean>) {
  const resource = useNotificationRulesResource(active);
  const editor = useNotificationRuleEditor({
    providers: resource.providers,
    rules: resource.rules,
    hasProviders: resource.hasProviders,
    loadData: resource.loadData,
    resolveProviderDefinitionById: resource.resolveProviderDefinitionById,
    formatEventTypeLabel: resource.formatEventTypeLabel,
    formatGroupByLabel: resource.formatGroupByLabel,
    buildRuleDisplayName: resource.buildRuleDisplayName,
  });

  return {
    ...resource,
    ...editor,
  };
}
