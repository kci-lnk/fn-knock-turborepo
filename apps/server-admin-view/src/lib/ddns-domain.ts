export type DDNSDomainTargetMode = "single" | "single_or_wildcard_root_pair";

export type DDNSDomainTargetRootField = "root_domain" | "site_name";

export interface DDNSDomainTargetsCapability {
  mode: DDNSDomainTargetMode;
  rootField?: DDNSDomainTargetRootField;
}

export type DDNSDomainTargetErrorCode =
  | "empty"
  | "invalid_domain"
  | "too_many_targets"
  | "duplicate_targets"
  | "invalid_pair"
  | "pair_unsupported"
  | "root_mismatch";

export interface DDNSDomainTargetParseSuccess {
  ok: true;
  canonical: string;
  targets: string[];
  pairBase: string | null;
}

export interface DDNSDomainTargetParseFailure {
  ok: false;
  canonical: string;
  targets: string[];
  error: DDNSDomainTargetErrorCode;
}

export type DDNSDomainTargetParseResult =
  | DDNSDomainTargetParseSuccess
  | DDNSDomainTargetParseFailure;

export interface DDNSDomainTargetValidationOptions {
  capability?: DDNSDomainTargetsCapability | null;
  rootDomain?: string | null;
}

const DOMAIN_TARGET_SEPARATOR = /[,\uFF0C\p{White_Space}]+/u;
const ASCII_DOMAIN_LABEL = /^[a-z0-9-]+$/;

const toASCIILowercase = (value: string) =>
  value.replace(/[A-Z]/g, (character) =>
    String.fromCharCode(character.charCodeAt(0) + 32),
  );

const normalizeDomainTargetToken = (value: string) =>
  toASCIILowercase(value).replace(/\.+$/u, "");

const splitDomainTargetInput = (value: unknown) =>
  String(value ?? "")
    .split(DOMAIN_TARGET_SEPARATOR)
    .filter((token) => token.length > 0)
    .map(normalizeDomainTargetToken);

const isIPv4Literal = (value: string) => {
  const parts = value.split(".");
  return (
    parts.length === 4 &&
    parts.every((part) => {
      if (!/^\d{1,3}$/.test(part)) {
        return false;
      }
      const number = Number(part);
      return number >= 0 && number <= 255;
    })
  );
};

const getWildcardBase = (target: string) =>
  target.startsWith("*.") ? target.slice(2) : null;

export const isSameOrSubdomain = (domain: string, zoneRoot: string) =>
  Boolean(
    domain &&
    zoneRoot &&
    (domain === zoneRoot || domain.endsWith(`.${zoneRoot}`)),
  );

export const isValidDDNSDomainTarget = (target: string) => {
  // eslint-disable-next-line no-control-regex -- this validation intentionally rejects non-ASCII input, including NUL.
  if (!target || target.length > 253 || !/^[\x00-\x7F]+$/.test(target)) {
    return false;
  }

  const wildcardBase = getWildcardBase(target);
  const domain = wildcardBase ?? target;
  if (!domain || domain.includes("*") || isIPv4Literal(domain)) {
    return false;
  }

  const labels = domain.split(".");
  if (labels.length < 2) {
    return false;
  }

  return labels.every(
    (label) =>
      label.length >= 1 &&
      label.length <= 63 &&
      ASCII_DOMAIN_LABEL.test(label) &&
      !label.startsWith("-") &&
      !label.endsWith("-"),
  );
};

export const normalizeDDNSDomainTargetInput = (value: unknown) => {
  const targets = splitDomainTargetInput(value);
  if (targets.length === 2) {
    const wildcardTarget = targets.find((target) => getWildcardBase(target));
    const rootTarget = targets.find((target) => !getWildcardBase(target));
    if (
      wildcardTarget &&
      rootTarget &&
      getWildcardBase(wildcardTarget) === rootTarget
    ) {
      return `${wildcardTarget},${rootTarget}`;
    }
  }
  return targets.join(",");
};

export const parseDDNSDomainTargets = (
  value: unknown,
): DDNSDomainTargetParseResult => {
  const targets = splitDomainTargetInput(value);
  const canonical = normalizeDDNSDomainTargetInput(value);

  if (targets.length === 0) {
    return { ok: false, canonical, targets, error: "empty" };
  }
  if (targets.length > 2) {
    return { ok: false, canonical, targets, error: "too_many_targets" };
  }
  if (!targets.every(isValidDDNSDomainTarget)) {
    return { ok: false, canonical, targets, error: "invalid_domain" };
  }
  if (new Set(targets).size !== targets.length) {
    return { ok: false, canonical, targets, error: "duplicate_targets" };
  }

  if (targets.length === 1) {
    return {
      ok: true,
      canonical: targets[0] ?? "",
      targets,
      pairBase: null,
    };
  }

  const wildcardTarget = targets.find((target) => getWildcardBase(target));
  const rootTarget = targets.find((target) => !getWildcardBase(target));
  if (
    !wildcardTarget ||
    !rootTarget ||
    getWildcardBase(wildcardTarget) !== rootTarget
  ) {
    return { ok: false, canonical, targets, error: "invalid_pair" };
  }

  return {
    ok: true,
    canonical: `${wildcardTarget},${rootTarget}`,
    targets: [wildcardTarget, rootTarget],
    pairBase: rootTarget,
  };
};

export const validateDDNSDomainTargets = (
  value: unknown,
  options: DDNSDomainTargetValidationOptions = {},
): DDNSDomainTargetParseResult => {
  const result = parseDDNSDomainTargets(value);
  if (!result.ok || result.pairBase === null) {
    return result;
  }

  if (options.capability?.mode !== "single_or_wildcard_root_pair") {
    return {
      ok: false,
      canonical: result.canonical,
      targets: result.targets,
      error: "pair_unsupported",
    };
  }

  if (options.capability.rootField) {
    const rootResult = parseDDNSDomainTargets(options.rootDomain);
    if (
      !rootResult.ok ||
      rootResult.targets.length !== 1 ||
      rootResult.targets[0]?.startsWith("*.") ||
      !isSameOrSubdomain(result.pairBase, rootResult.targets[0] ?? "")
    ) {
      return {
        ok: false,
        canonical: result.canonical,
        targets: result.targets,
        error: "root_mismatch",
      };
    }
  }

  return result;
};
