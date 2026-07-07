export type ProxyPathForwardingMode = "keep" | "strip";

export type ProxyPathForwardingPreview = {
  requestPath: string;
  upstreamPath: string;
};

const ensureLeadingSlash = (value: string): string =>
  value.startsWith("/") ? value : `/${value}`;

const singleJoiningSlash = (left: string, right: string): string => {
  const leftHasSlash = left.endsWith("/");
  const rightHasSlash = right.startsWith("/");
  if (leftHasSlash && rightHasSlash) return left + right.slice(1);
  if (!leftHasSlash && !rightHasSlash) return `${left}/${right}`;
  return left + right;
};

export const cleanProxyRoutePath = (value: string): string => {
  const raw = value.trim();
  if (!raw.startsWith("/")) return raw;

  const segments: string[] = [];
  for (const segment of raw.split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") {
      segments.pop();
      continue;
    }
    segments.push(segment);
  }

  return `/${segments.join("/")}`;
};

export const resolveProxyTargetPath = (target: string): string => {
  const raw = target.trim();
  if (!raw) return "";

  try {
    const parsed = new URL(raw);
    return parsed.pathname || "";
  } catch {
    return "";
  }
};

export const stripMatchedRoutePath = (
  requestPath: string,
  routePath: string,
): string => {
  const normalizedRequestPath = ensureLeadingSlash(requestPath);
  if (!routePath || routePath === "/") return normalizedRequestPath;

  const nextPath = normalizedRequestPath.startsWith(routePath)
    ? normalizedRequestPath.slice(routePath.length)
    : normalizedRequestPath;
  return ensureLeadingSlash(nextPath);
};

export const buildProxyForwardedPath = (
  targetPath: string,
  requestPath: string,
  mode: ProxyPathForwardingMode,
  routePath: string,
): string => {
  const routeOutputPath =
    mode === "strip"
      ? stripMatchedRoutePath(requestPath, routePath)
      : ensureLeadingSlash(requestPath);
  return targetPath
    ? singleJoiningSlash(targetPath, routeOutputPath)
    : routeOutputPath;
};

export const buildProxyPathForwardingPreview = ({
  routePath,
  target,
  mode,
  sampleSuffix = "/example",
}: {
  routePath: string;
  target: string;
  mode: ProxyPathForwardingMode;
  sampleSuffix?: string;
}): ProxyPathForwardingPreview => {
  const cleanedRoutePath = cleanProxyRoutePath(routePath);
  const requestSuffix = ensureLeadingSlash(sampleSuffix.trim() || "/example");
  const requestPath =
    cleanedRoutePath && cleanedRoutePath !== "/"
      ? singleJoiningSlash(cleanedRoutePath, requestSuffix)
      : requestSuffix;

  return {
    requestPath,
    upstreamPath: buildProxyForwardedPath(
      resolveProxyTargetPath(target),
      requestPath,
      mode,
      cleanedRoutePath,
    ),
  };
};
