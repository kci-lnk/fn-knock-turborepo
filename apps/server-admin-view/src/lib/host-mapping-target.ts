import type {
  HostMapping,
  HostMappingStaticServe,
  HostMappingTargetType,
} from "@/types";
import {
  isHttpProxyTargetProtocol,
  isSupportedProxyTargetUrl,
} from "@admin-shared/utils/proxyTargetInput";

export const DEFAULT_STATIC_INDEX_FILES = ["index.html", "index.htm"];
export const STATIC_INDEX_FILES_LIMIT = 16;
export const STATIC_INDEX_FILE_NAME_LIMIT = 255;

export type StaticServeValidationIssue =
  | "path_required"
  | "path_not_absolute"
  | "path_has_parent_segment"
  | "path_unsafe"
  | "invalid_index_file"
  | "duplicate_index_file"
  | "too_many_index_files";

export const normalizeHostMappingTargetType = (
  value: unknown,
): HostMappingTargetType =>
  value === "file" || value === "directory" ? value : "proxy";

export const isProxyHostMapping = (
  mapping: Pick<HostMapping, "target_type"> | { target_type?: unknown },
): boolean => normalizeHostMappingTargetType(mapping.target_type) === "proxy";

export const isStaticHostMapping = (
  mapping: Pick<HostMapping, "target_type"> | { target_type?: unknown },
): boolean => normalizeHostMappingTargetType(mapping.target_type) !== "proxy";

export const isStaticDirectoryHostMapping = (
  mapping: Pick<HostMapping, "target_type"> | { target_type?: unknown },
): boolean =>
  normalizeHostMappingTargetType(mapping.target_type) === "directory";

export const createDefaultStaticServe = (
  targetType: HostMappingTargetType,
): HostMappingStaticServe => ({
  path: "",
  index_files:
    targetType === "directory" ? [...DEFAULT_STATIC_INDEX_FILES] : [],
  directory_listing: { enabled: false, render_readme: false },
});

export const isValidStaticIndexFileName = (value: string): boolean => {
  const normalized = value.trim();
  return (
    normalized.length > 0 &&
    new TextEncoder().encode(normalized).byteLength <=
      STATIC_INDEX_FILE_NAME_LIMIT &&
    normalized !== "." &&
    normalized !== ".." &&
    !normalized.startsWith(".") &&
    !normalized.includes("/") &&
    !normalized.includes("\\") &&
    !/[\p{Cc}\p{Cf}]/u.test(normalized)
  );
};

export const normalizeStaticIndexFiles = (
  values: readonly string[],
): string[] => {
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const value of values) {
    const filename = value.trim();
    if (!filename || seen.has(filename)) continue;
    seen.add(filename);
    normalized.push(filename);
    if (normalized.length >= STATIC_INDEX_FILES_LIMIT) break;
  }
  return normalized;
};

export const normalizeHostMappingStaticServe = (
  targetType: HostMappingTargetType,
  value?: Partial<HostMappingStaticServe> | null,
): HostMappingStaticServe | null => {
  if (targetType === "proxy") return null;
  const defaults = createDefaultStaticServe(targetType);
  const directoryListing = value?.directory_listing;
  const listingEnabled =
    targetType === "directory" && directoryListing?.enabled === true;
  return {
    path: typeof value?.path === "string" ? value.path.trim() : "",
    index_files:
      targetType === "directory"
        ? Array.isArray(value?.index_files)
          ? normalizeStaticIndexFiles(value.index_files)
          : defaults.index_files
        : [],
    directory_listing: {
      enabled: listingEnabled,
      render_readme: listingEnabled && directoryListing?.render_readme === true,
    },
  };
};

export const isAbsoluteServerPath = (
  value: string,
  isWindows?: boolean,
): boolean => {
  const path = value.trim();
  const posixAbsolute = path.startsWith("/");
  const windowsPath = path.replace(/\//gu, "\\");
  const windowsAbsolute =
    /^[A-Za-z]:\\/u.test(windowsPath) ||
    /^\\\\[^\\]+\\[^\\]+/u.test(windowsPath);
  if (isWindows === true) return windowsAbsolute;
  if (isWindows === false) return posixAbsolute;
  return posixAbsolute || windowsAbsolute;
};

const isSafeWindowsVisibleName = (value: string): boolean => {
  if (/[<>:"|?*]/u.test(value) || value.endsWith(" ") || value.endsWith(".")) {
    return false;
  }
  const stem = (value.split(".")[0] ?? "").toUpperCase();
  if (
    ["CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$"].includes(stem)
  ) {
    return false;
  }
  return !/^(?:COM|LPT)(?:[1-9¹²³])$/u.test(stem);
};

const isUnsafeStaticTargetPath = (
  path: string,
  isWindows?: boolean,
): boolean => {
  if (/[\p{Cc}\p{Cf}]/u.test(path)) return true;
  if (/^\/+$/u.test(path) || /^[A-Za-z]:[\\/]*$/u.test(path)) return true;
  const normalizedWindowsPath = path.replace(/\//gu, "\\");
  if (normalizedWindowsPath.startsWith("\\\\")) return true;

  const segments = path.split(/[\\/]+/u).filter(Boolean);
  const targetName = segments[segments.length - 1] ?? "";
  if (!targetName || targetName.startsWith(".")) return true;

  // A backslash is a legal separator on Windows but an unsafe literal name
  // character in the Go gateway's POSIX static-root validation.
  const windowsPath = isWindows ?? !path.startsWith("/");
  if (
    windowsPath &&
    (/^(?:\\\\|\/\/)[.?][\\/]/u.test(path) ||
      !isSafeWindowsVisibleName(targetName))
  ) {
    return true;
  }
  return !windowsPath && path.includes("\\");
};

export const getStaticServeValidationIssue = ({
  isWindows,
  staticServe,
  targetType,
}: {
  isWindows?: boolean;
  staticServe: HostMappingStaticServe | null | undefined;
  targetType: HostMappingTargetType;
}): StaticServeValidationIssue | null => {
  if (targetType === "proxy") return null;
  const path = staticServe?.path?.trim() ?? "";
  if (!path) return "path_required";
  if (!isAbsoluteServerPath(path, isWindows)) return "path_not_absolute";
  if (path.split(/[\\/]+/u).includes("..") || path.includes("\0")) {
    return "path_has_parent_segment";
  }
  if (isUnsafeStaticTargetPath(path, isWindows)) return "path_unsafe";
  if (
    targetType === "directory" &&
    (staticServe?.index_files?.length ?? 0) > STATIC_INDEX_FILES_LIMIT
  ) {
    return "too_many_index_files";
  }
  if (targetType === "directory") {
    const indexFiles = staticServe?.index_files ?? [];
    if (
      !indexFiles.every(
        (value) =>
          isValidStaticIndexFileName(value) &&
          (isWindows !== true || isSafeWindowsVisibleName(value.trim())),
      )
    ) {
      return "invalid_index_file";
    }
    const normalizedNames = indexFiles.map((item) => item.trim());
    if (new Set(normalizedNames).size !== normalizedNames.length) {
      return "duplicate_index_file";
    }
  }
  return null;
};

export const getHostMappingTargetText = (
  mapping: Pick<HostMapping, "target" | "target_type" | "static_serve">,
): string =>
  isProxyHostMapping(mapping)
    ? mapping.target.trim()
    : (mapping.static_serve?.path?.trim() ?? "");

export const getLocationRulesCount = (mapping: HostMapping): number =>
  isProxyHostMapping(mapping) ? (mapping.locations?.length ?? 0) : 0;

export const canRefreshHostMappingMetadata = (target: string): boolean => {
  const normalizedTarget = target.trim();
  if (!isSupportedProxyTargetUrl(normalizedTarget)) return false;
  try {
    return isHttpProxyTargetProtocol(new URL(normalizedTarget).protocol);
  } catch {
    return false;
  }
};
