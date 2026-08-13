import type { HostMapping } from "@/types";
import { normalizeHostLike } from "./subdomain-host-model";

export const normalizeDisabledHosts = (
  hosts: string[] | undefined | null,
): string[] => [
  ...new Set((hosts ?? []).map(normalizeHostLike).filter(Boolean)),
];

export const hasSameDisabledHosts = (
  left: string[] | undefined | null,
  right: string[] | undefined | null,
): boolean => {
  const leftHosts = normalizeDisabledHosts(left);
  const rightHosts = normalizeDisabledHosts(right);
  return (
    leftHosts.length === rightHosts.length &&
    leftHosts.every((host, index) => host === rightHosts[index])
  );
};

export const mergeGatewayDisabledHostsForMapping = (
  currentDisabledHosts: string[],
  previousHosts: string[],
  nextHost: string,
  enabledForNextHost: boolean,
): string[] => {
  const disabledHosts = new Set(normalizeDisabledHosts(currentDisabledHosts));
  const normalizedNextHost = normalizeHostLike(nextHost);

  for (const host of normalizeDisabledHosts(previousHosts)) {
    disabledHosts.delete(host);
  }

  if (normalizedNextHost) {
    if (enabledForNextHost) {
      disabledHosts.delete(normalizedNextHost);
    } else {
      disabledHosts.add(normalizedNextHost);
    }
  }

  return [...disabledHosts];
};

export const hasSameMappingOrder = (
  left: HostMapping[],
  right: HostMapping[],
) =>
  left.length === right.length &&
  left.every((mapping, index) => mapping.host === right[index]?.host);

export const mergeFilteredMappingsOrder = ({
  allMappings,
  filteredMappings,
  isPinnedMapping,
  nextFiltered,
  visibleMappings,
}: {
  allMappings: HostMapping[];
  filteredMappings: HostMapping[];
  isPinnedMapping?: (mapping: HostMapping) => boolean;
  nextFiltered: HostMapping[];
  visibleMappings: HostMapping[];
}): HostMapping[] => {
  const filteredHostSet = new Set(filteredMappings.map((item) => item.host));
  let nextFilteredIndex = 0;
  const nextVisible = visibleMappings.map((mapping) =>
    filteredHostSet.has(mapping.host)
      ? (nextFiltered[nextFilteredIndex++] ?? mapping)
      : mapping,
  );

  let nextVisibleIndex = 0;
  return allMappings.map((mapping) =>
    (isPinnedMapping?.(mapping) ?? mapping.service_role === "auth")
      ? mapping
      : (nextVisible[nextVisibleIndex++] ?? mapping),
  );
};
