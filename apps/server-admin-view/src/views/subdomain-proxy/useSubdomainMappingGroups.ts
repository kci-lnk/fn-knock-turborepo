import type { ComputedRef } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { HostMapping, HostMappingGroup } from "@/types";
import {
  applyHostMappingGroupSections,
  moveHostMappingsToGroup,
  resolveHostMappingGroupSaveFeedback,
  type HostMappingGroupSaveFeedback,
  type HostMappingGroupSection,
} from "./host-mapping-groups";

type RunAsyncAction = <T>(action: () => Promise<T>) => Promise<T | undefined>;

const groupSaveToastKeys: Record<HostMappingGroupSaveFeedback, string> = {
  created: "admin.subdomainProxy.groupCreated",
  renamed: "admin.subdomainProxy.groupRenamed",
  deleted: "admin.subdomainProxy.groupDeleted",
  reordered: "admin.subdomainProxy.groupOrderUpdated",
  saved: "admin.subdomainProxy.groupsSaved",
};

const hasSameMappingOrderAndGroups = (
  currentMappings: HostMapping[],
  nextMappings: HostMapping[],
): boolean =>
  currentMappings.length === nextMappings.length &&
  currentMappings.every(
    (mapping, index) =>
      mapping.host === nextMappings[index]?.host &&
      mapping.group_id === nextMappings[index]?.group_id,
  );

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
    const feedback = resolveHostMappingGroupSaveFeedback(
      groups.value,
      nextGroups,
    );
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
      toast.success(translate(groupSaveToastKeys[feedback]));
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
    if (hasSameMappingOrderAndGroups(allMappings.value, nextMappings)) return;

    const previousGroupsByHost = new Map(
      allMappings.value.map((mapping) => [mapping.host, mapping.group_id]),
    );
    const movedAcrossGroups = nextMappings.some(
      (mapping) => previousGroupsByHost.get(mapping.host) !== mapping.group_id,
    );
    await runSaveMappings(async () => {
      await saveCatalog(nextMappings, groups.value);
      toast.success(
        translate(
          movedAcrossGroups
            ? "admin.subdomainProxy.mappingsMoved"
            : "admin.subdomainProxy.groupedMappingOrderUpdated",
        ),
      );
      return true;
    });
  };

  const moveMappingsToGroup = async (
    hosts: string[],
    groupId: string | null,
  ): Promise<boolean> => {
    const nextMappings = moveHostMappingsToGroup(
      allMappings.value,
      new Set(hosts),
      groupId,
    );
    if (hasSameMappingOrderAndGroups(allMappings.value, nextMappings)) {
      return false;
    }

    const targetGroupName =
      (groupId
        ? groups.value.find((group) => group.id === groupId)?.name
        : translate("admin.subdomainProxy.ungrouped")) || undefined;
    const saved = await runSaveMappings(async () => {
      await saveCatalog(nextMappings, groups.value);
      toast.success(translate("admin.subdomainProxy.mappingsMoved"), {
        description: targetGroupName,
      });
      return true;
    });
    return saved === true;
  };

  const updateHostMappingGroupedView = async (value: boolean) => {
    if (value === groupedView.value) return;
    await runSaveMappings(async () => {
      await saveCatalog(allMappings.value, groups.value, value);
      toast.success(
        translate(
          value
            ? "admin.subdomainProxy.groupedViewEnabled"
            : "admin.subdomainProxy.groupedViewDisabled",
        ),
      );
      return true;
    });
  };

  return {
    moveMappingsToGroup,
    saveGroupedMappingOrder,
    saveMappingGroups,
    updateHostMappingGroupedView,
  };
};
