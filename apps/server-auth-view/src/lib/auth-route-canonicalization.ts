const AUTH_BASE_PREFIXES = ["/__auth__", "/auth"] as const;

const LEGACY_AUTH_HASH_PATHS = new Set([
  "/",
  "/login",
  "/oidc/bind",
  "/ldap/bind",
]);

export interface AuthBrowserLocation {
  pathname: string;
  search: string;
  hash: string;
}

function normalizedPathname(pathname: string) {
  if (pathname.length > 1 && pathname.endsWith("/")) {
    return pathname.replace(/\/+$/, "");
  }
  return pathname || "/";
}

function authBasePrefix(pathname: string) {
  const normalized = normalizedPathname(pathname);
  return (
    AUTH_BASE_PREFIXES.find(
      (prefix) => normalized === prefix || normalized.startsWith(`${prefix}/`),
    ) ?? ""
  );
}

function canonicalizeDuplicatedPrefix(pathname: string) {
  const replacements: Array<[string, string]> = [
    ["/__auth__/__auth__", "/__auth__"],
    ["/auth/auth", "/auth"],
  ];

  for (const [duplicatedPrefix, canonicalPrefix] of replacements) {
    if (
      pathname === duplicatedPrefix ||
      pathname.startsWith(`${duplicatedPrefix}/`)
    ) {
      return `${canonicalPrefix}${pathname.slice(duplicatedPrefix.length)}`;
    }
  }
  return pathname;
}

function isAuthHashRouteContainer(pathname: string) {
  const normalized = normalizedPathname(pathname);
  return (
    normalized === "/" || normalized === "/auth" || normalized === "/__auth__"
  );
}

function mergeSearchParams(outerSearch: string, hashSearch: string) {
  const merged = new URLSearchParams(outerSearch);
  for (const [key, value] of new URLSearchParams(hashSearch)) {
    merged.append(key, value);
  }
  const encoded = merged.toString();
  return encoded ? `?${encoded}` : "";
}

/**
 * Migrates only known routes from the auth app's former hash router. Business
 * hashes such as `#/whitelist` belong to another application and must retain
 * their origin and hash identity.
 */
export function canonicalAuthHistoryTarget({
  pathname: rawPathname,
  search,
  hash,
}: AuthBrowserLocation): string | null {
  let pathname = canonicalizeDuplicatedPrefix(rawPathname || "/");
  let nextSearch = search;
  let nextHash = hash;
  let changed = pathname !== rawPathname;

  if (hash.startsWith("#/") && isAuthHashRouteContainer(pathname)) {
    let hashRoute: URL;
    try {
      hashRoute = new URL(hash.slice(1), "https://auth.invalid/");
    } catch {
      return changed ? `${pathname}${nextSearch}${nextHash}` : null;
    }

    const hashPathname = normalizedPathname(hashRoute.pathname);
    if (LEGACY_AUTH_HASH_PATHS.has(hashPathname)) {
      const basePrefix = authBasePrefix(pathname);
      pathname = basePrefix
        ? `${basePrefix}${hashRoute.pathname}`
        : hashRoute.pathname;
      nextSearch = mergeSearchParams(search, hashRoute.search);
      nextHash = hashRoute.hash;
      changed = true;
    }
  }

  return changed ? `${pathname}${nextSearch}${nextHash}` : null;
}
