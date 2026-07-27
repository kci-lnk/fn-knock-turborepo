import type { HostMapping, HostMappingGroup } from "@/types";

export const UNGROUPED_SECTION_KEY = "__ungrouped__";

export interface HostMappingGroupSection {
  key: string;
  groupId: string | null;
  name: string;
  mappings: HostMapping[];
  isUngrouped: boolean;
}

export type HostMappingGroupSaveFeedback =
  | "created"
  | "renamed"
  | "deleted"
  | "reordered"
  | "saved";

export const normalizeHostMappingGroupNameKey = (name: string): string =>
  name.trim().toLowerCase();

export const isHostMappingGroupNameLengthValid = (name: string): boolean => {
  const length = [...name.trim()].length;
  return length >= 1 && length <= 40;
};

export const buildHostMappingDragRenderKey = (
  mappings: HostMapping[],
): string => JSON.stringify(mappings.map((mapping) => mapping.host));

interface HostMappingGroupCrypto {
  randomUUID?: () => string;
  getRandomValues?: (array: Uint8Array) => Uint8Array;
}

export const createHostMappingGroupId = (
  cryptoApi: HostMappingGroupCrypto | null = typeof globalThis.crypto ===
  "undefined"
    ? null
    : globalThis.crypto,
): string => {
  if (typeof cryptoApi?.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }

  const bytes = new Uint8Array(16);
  if (typeof cryptoApi?.getRandomValues === "function") {
    cryptoApi.getRandomValues(bytes);
  } else {
    for (let index = 0; index < bytes.length; index += 1) {
      bytes[index] = Math.floor(Math.random() * 256);
    }
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x40;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;

  const hex = Array.from(bytes, (value) =>
    value.toString(16).padStart(2, "0"),
  ).join("");
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20),
  ].join("-");
};

export const buildHostMappingGroupSections = (
  mappings: HostMapping[],
  groups: HostMappingGroup[],
  ungroupedName: string,
  includeEmptyGroups = true,
): HostMappingGroupSection[] => {
  if (groups.length === 0) {
    return [
      {
        key: UNGROUPED_SECTION_KEY,
        groupId: null,
        name: "",
        mappings: [...mappings],
        isUngrouped: true,
      },
    ];
  }

  const validGroupIds = new Set(groups.map((group) => group.id));
  const sections = groups.map<HostMappingGroupSection>((group) => ({
    key: group.id,
    groupId: group.id,
    name: group.name,
    mappings: mappings.filter((mapping) => mapping.group_id === group.id),
    isUngrouped: false,
  }));
  const ungrouped = mappings.filter(
    (mapping) => !mapping.group_id || !validGroupIds.has(mapping.group_id),
  );

  const visibleSections = includeEmptyGroups
    ? sections
    : sections.filter((section) => section.mappings.length > 0);
  if (ungrouped.length > 0 || includeEmptyGroups) {
    visibleSections.push({
      key: UNGROUPED_SECTION_KEY,
      groupId: null,
      name: ungroupedName,
      mappings: ungrouped,
      isUngrouped: true,
    });
  }
  return visibleSections;
};

export const applyHostMappingGroupSections = (
  allMappings: HostMapping[],
  sections: HostMappingGroupSection[],
  isAuthServiceTarget: (target: string) => boolean,
): HostMapping[] => {
  const orderedRegularMappings = sections.flatMap((section) =>
    section.mappings.map((mapping) => ({
      ...mapping,
      group_id: section.groupId,
    })),
  );
  let regularIndex = 0;

  return allMappings.map((mapping) => {
    if (isAuthServiceTarget(mapping.target)) {
      return { ...mapping, group_id: null };
    }
    const replacement = orderedRegularMappings[regularIndex];
    regularIndex += 1;
    return replacement ?? mapping;
  });
};

export const moveHostMappingsToGroup = (
  mappings: HostMapping[],
  hosts: ReadonlySet<string>,
  groupId: string | null,
): HostMapping[] =>
  mappings.map((mapping) =>
    hosts.has(mapping.host) ? { ...mapping, group_id: groupId } : mapping,
  );

export const resolveHostMappingGroupSaveFeedback = (
  previousGroups: HostMappingGroup[],
  nextGroups: HostMappingGroup[],
): HostMappingGroupSaveFeedback => {
  const previousById = new Map(
    previousGroups.map((group) => [group.id, group]),
  );
  const nextById = new Map(nextGroups.map((group) => [group.id, group]));
  const changes = new Set<Exclude<HostMappingGroupSaveFeedback, "saved">>();

  if (nextGroups.some((group) => !previousById.has(group.id))) {
    changes.add("created");
  }
  if (previousGroups.some((group) => !nextById.has(group.id))) {
    changes.add("deleted");
  }
  if (
    nextGroups.some(
      (group) =>
        previousById.has(group.id) &&
        previousById.get(group.id)?.name !== group.name,
    )
  ) {
    changes.add("renamed");
  }

  const previousCommonIds = previousGroups
    .filter((group) => nextById.has(group.id))
    .map((group) => group.id);
  const nextCommonIds = nextGroups
    .filter((group) => previousById.has(group.id))
    .map((group) => group.id);
  if (
    previousCommonIds.some((groupId, index) => nextCommonIds[index] !== groupId)
  ) {
    changes.add("reordered");
  }

  if (changes.size !== 1) return "saved";
  return [...changes][0] ?? "saved";
};
