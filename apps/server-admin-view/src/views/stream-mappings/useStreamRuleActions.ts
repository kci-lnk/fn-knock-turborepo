import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import { sameStreamRule } from "@/lib/stream-mappings";
import type { StreamMapping } from "@/types";
import { getMappingKey, type StreamRuleAction } from "./streamMappingModel";

export function useStreamRuleActions(
  visible: ComputedRef<StreamMapping[]>,
  search: Ref<string>,
  save: (
    action: StreamRuleAction,
    rules: readonly StreamMapping[],
  ) => Promise<boolean>,
) {
  const selectionMode = ref(false);
  const selected = ref(new Set<string>());
  const saving = ref(false);
  const pending = ref<{
    action: StreamRuleAction;
    rules: StreamMapping[];
    batch: boolean;
  } | null>(null);

  const clearSelection = () => selected.value.clear();
  watch(search, clearSelection);
  watch(visible, (mappings, previous) => {
    const current = new Map(
      mappings.map((mapping) => [getMappingKey(mapping), mapping]),
    );
    for (const rule of previous) {
      const key = getMappingKey(rule);
      const latest = current.get(key);
      if (!latest || latest.target !== rule.target) selected.value.delete(key);
    }
  });
  function toggleSelectionMode() {
    selectionMode.value = !selectionMode.value;
    clearSelection();
  }
  function setSelected(key: string, checked: boolean) {
    if (checked) selected.value.add(key);
    else selected.value.delete(key);
  }
  function setAllVisible(checked: boolean) {
    selected.value = new Set(checked ? visible.value.map(getMappingKey) : []);
  }
  const allChecked = computed(() => {
    if (selected.value.size === 0) return false;
    return selected.value.size === visible.value.length
      ? true
      : "indeterminate";
  });

  const selectedRules = computed(() =>
    visible.value.filter((mapping) =>
      selected.value.has(getMappingKey(mapping)),
    ),
  );
  function affected(mapping: StreamMapping, action: StreamRuleAction) {
    if (action === "delete") return true;
    return (mapping.manually_disabled === true) === (action === "enable");
  }
  function requestAction(action: StreamRuleAction, mapping?: StreamMapping) {
    const rules = (mapping ? [mapping] : selectedRules.value).filter((item) =>
      affected(item, action),
    );
    if (!rules.length || saving.value) return;
    pending.value = { action, rules, batch: !mapping };
  }
  watch([visible, saving], ([mappings]) => {
    const request = pending.value;
    if (!request || saving.value) return;
    const currentByKey = new Map(
      mappings.map((mapping) => [getMappingKey(mapping), mapping]),
    );
    if (
      request.rules.some((rule) => {
        const current = currentByKey.get(getMappingKey(rule));
        return (
          !current ||
          !sameStreamRule(current, rule) ||
          !affected(current, request.action)
        );
      })
    )
      cancelAction();
  });
  function cancelAction() {
    if (!saving.value) pending.value = null;
  }
  async function confirmAction() {
    const request = pending.value;
    if (!request || saving.value) return;
    saving.value = true;
    try {
      if (await save(request.action, request.rules)) {
        pending.value = null;
        if (request.batch) clearSelection();
      }
    } finally {
      saving.value = false;
    }
  }
  return {
    selectionMode,
    selected,
    selectedRules,
    saving,
    allChecked,
    pending,
    clearSelection,
    toggleSelectionMode,
    setSelected,
    setAllVisible,
    affected,
    requestAction,
    cancelAction,
    confirmAction,
  };
}
