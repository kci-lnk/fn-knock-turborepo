import type { HostMapping } from "../types";

const normalizeComparableBasicAuth = (
  value: HostMapping["basic_auth"],
): HostMapping["basic_auth"] => {
  const username = value.username.trim();
  const password = value.password;
  if (
    value.enabled !== true ||
    !username ||
    !password ||
    username.includes(":")
  ) {
    return {
      enabled: false,
      username: "",
      password: "",
    };
  }

  return {
    enabled: true,
    username,
    password,
  };
};

const hasUsableBasicAuth = (value: HostMapping["basic_auth"]): boolean =>
  normalizeComparableBasicAuth(value).enabled;

const basicAuthMatches = (
  left: HostMapping["basic_auth"],
  right: HostMapping["basic_auth"],
): boolean => {
  const normalizedLeft = normalizeComparableBasicAuth(left);
  const normalizedRight = normalizeComparableBasicAuth(right);
  return (
    normalizedLeft.enabled === normalizedRight.enabled &&
    normalizedLeft.username === normalizedRight.username &&
    normalizedLeft.password === normalizedRight.password
  );
};

const hostKey = (value: string): string => value.trim().toLowerCase();

export const hasPendingHostMappingMetadata = (
  mappings: HostMapping[],
  previousMappings: HostMapping[] | null = null,
): boolean => {
  const previousByHost = previousMappings
    ? new Map(previousMappings.map((mapping) => [hostKey(mapping.host), mapping]))
    : null;

  return mappings.some((mapping) => {
    if (!mapping.target.trim()) return false;
    if (!mapping.title.trim() || !mapping.favicon.trim()) return true;
    if (!previousByHost || !hasUsableBasicAuth(mapping.basic_auth)) {
      return false;
    }

    const previous = previousByHost.get(hostKey(mapping.host));
    return (
      !previous ||
      previous.target.trim() !== mapping.target.trim() ||
      !basicAuthMatches(previous.basic_auth, mapping.basic_auth)
    );
  });
};
