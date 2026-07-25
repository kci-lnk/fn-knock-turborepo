import type { ComputedRef } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { HostMapping, HostMappingGroup } from "@/types";
import {
  applyHostMappingGroupSections,
  moveHostMappingsToGroup,
  type HostMappingGroupSection,
} from "./host-mapping-groups";

type RunAsyncAction = <T>(action: () => Promise<T>) => Promise<T | undefined>;

export const useSubdomainMappingGroups = ({
  allMappings,
  groupedView,
  groups,
  isAuthServiceTarget,
  runSaveMappings,
  saveCatalog,
  translate,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  groupedView: ComputedRef<boolean>;
  groups: ComputedRef<HostMappingGroup[]>;
  isAuthServiceTarget: (target: string) => boolean;
  runSaveMappings: RunAsyncAction;
  saveCatalog: (
    mappings: HostMapping[],
    groups: HostMappingGroup[],
    groupedView?: boolean,
  ) => Promise<unknown>;
  translate: (key: string) => string;
}) => {
  const saveMappingGroups = async (
    nextGroups: HostMappingGroup[],
    onComplete?: (saved: boolean) => void,
  ) => {
    const validIds = new Set(nextGroups.map((group) => group.id));
    const nextMappings = allMappings.value.map((mapping) => ({
      ...mapping,
      group_id:
        !isAuthServiceTarget(mapping.target) &&
        mapping.group_id &&
        validIds.has(mapping.group_id)
          ? mapping.group_id
          : null,
    }));
    const saved = await runSaveMappings(async () => {
      await saveCatalog(nextMappings, nextGroups);
      toast.success(translate("admin.subdomainProxy.groupsSaved"));
      return true;
    });
    onComplete?.(saved === true);
    return saved === true;
  };

  const saveGroupedMappingOrder = async (
    sections: HostMappingGroupSection[],
  ) => {
    const nextMappings = applyHostMappingGroupSections(
      allMappings.value,
      sections,
      isAuthServiceTarget,
    );
    await runSaveMappings(() => saveCatalog(nextMappings, groups.value));
  };

  const moveMappingsToGroup = async (
    hosts: string[],
    groupId: string | null,
  ) => {
    const nextMappings = moveHostMappingsToGroup(
      allMappings.value,
      new Set(hosts),
      groupId,
    );
    await runSaveMappings(() => saveCatalog(nextMappings, groups.value));
  };

  const updateHostMappingGroupedView = async (value: boolean) => {
    if (value === groupedView.value) return;
    await runSaveMappings(() =>
      saveCatalog(allMappings.value, groups.value, value),
    );
  };

  return {
    moveMappingsToGroup,
    saveGroupedMappingOrder,
    saveMappingGroups,
    updateHostMappingGroupedView,
  };
};
