import { Elysia } from "elysia";
import { stat, readFile, realpath } from "node:fs/promises";
import { extname, isAbsolute, relative, resolve, sep } from "node:path";

type StaticFilesPluginOptions = {
  root: string;
  mountPrefixes?: string[];
  denyDotFiles?: boolean;
};

const MIME_TYPES: Record<string, string> = {
  ".avif": "image/avif",
  ".css": "text/css; charset=utf-8",
  ".eot": "application/vnd.ms-fontobject",
  ".gif": "image/gif",
  ".html": "text/html; charset=utf-8",
  ".ico": "image/x-icon",
  ".jpeg": "image/jpeg",
  ".jpg": "image/jpeg",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".txt": "text/plain; charset=utf-8",
  ".wasm": "application/wasm",
  ".webp": "image/webp",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
};

const normalizePrefix = (value: string): string => {
  const trimmed = value.trim();
  if (trimmed.length === 0 || trimmed === "/") return "/";
  const withSlash = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  return withSlash.endsWith("/") ? withSlash.slice(0, -1) : withSlash;
};

const hasControlChars = (value: string): boolean => /[\x00-\x1f\x7f]/.test(value);

const hasDotSegments = (path: string): boolean => {
  const normalized = path.replace(/\\/g, "/");
  const segments = normalized.split("/").filter(Boolean);
  return segments.some((segment) => segment.startsWith("."));
};

const hasTraversalSegments = (path: string): boolean => {
  const normalized = path.replace(/\\/g, "/");
  const segments = normalized.split("/").filter(Boolean);
  return segments.some((segment) => segment === "..");
};

const tryDecode = (value: string): string => {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
};

const stripPrefix = (pathname: string, prefix: string): string | null => {
  if (prefix === "/") return pathname;
  if (pathname === prefix) return "/";
  if (pathname.startsWith(`${prefix}/`)) return pathname.slice(prefix.length);
  return null;
};

type RequestWithOriginalURL = Request & { fnOriginalUrl?: string };

const getRawPath = (request: Request): string => {
  const original =
    (request as RequestWithOriginalURL).fnOriginalUrl ??
    request.headers.get("x-fn-original-url");
  if (!original) return new URL(request.url).pathname;
  const queryIndex = original.indexOf("?");
  return queryIndex >= 0 ? original.slice(0, queryIndex) : original;
};

const resolveSafePath = (root: string, candidatePath: string): string | null => {
  if (!candidatePath.startsWith("/")) return null;
  if (hasControlChars(candidatePath)) return null;
  if (candidatePath.includes("\\")) return null;
  const relative = candidatePath.slice(1);
  if (!relative || relative.endsWith("/") || !relative.includes(".")) return null;

  const absolute = resolve(root, relative);
  if (absolute === root || absolute.startsWith(`${root}${sep}`)) return absolute;
  return null;
};

const isWithinRoot = (rootPath: string, filePath: string): boolean => {
  const rel = relative(rootPath, filePath);
  if (!rel) return true;
  if (rel.startsWith("..")) return false;
  return !isAbsolute(rel);
};

const getMimeType = (absolutePath: string): string => {
  const extension = extname(absolutePath).toLowerCase();
  return MIME_TYPES[extension] ?? "application/octet-stream";
};

const getCacheControl = (pathname: string): string => {
  // Fingerprinted bundles can be cached aggressively.
  if (/-[A-Za-z0-9]{8,}\./.test(pathname)) return "public, max-age=31536000, immutable";
  return "public, max-age=300";
};

export const createStaticFilesPlugin = ({
  root,
  mountPrefixes = ["/"],
  denyDotFiles = true,
}: StaticFilesPluginOptions) => {
  const absoluteRoot = resolve(root);
  const prefixes = mountPrefixes.map(normalizePrefix);
  let realRootPromise: Promise<string> | null = null;

  const getRealRoot = () => {
    if (!realRootPromise) {
      realRootPromise = realpath(absoluteRoot).catch(() => absoluteRoot);
    }
    return realRootPromise;
  };

  return new Elysia({ name: "plugin-static-files" }).onRequest(
    async ({ request }) => {
      if (request.method !== "GET" && request.method !== "HEAD") return;

      const pathname = tryDecode(new URL(request.url).pathname);
      const rawPathname = tryDecode(getRawPath(request));
      if (hasControlChars(pathname)) return;
      if (hasControlChars(rawPathname)) return new Response("Not Found", { status: 404 });
      if (hasTraversalSegments(rawPathname)) return new Response("Not Found", { status: 404 });
      if (pathname.startsWith("/api") || pathname === "/" || pathname === "/index.html") return;
      let matchedStaticLikePath = false;

      for (const prefix of prefixes) {
        const stripped = stripPrefix(pathname, prefix);
        if (stripped === null) continue;
        if (stripped.includes(".")) matchedStaticLikePath = true;
        if (denyDotFiles && hasDotSegments(stripped)) continue;

        const absoluteFilePath = resolveSafePath(absoluteRoot, stripped);
        if (!absoluteFilePath) continue;

        const [rootRealPath, fileRealPath] = await Promise.all([
          getRealRoot(),
          realpath(absoluteFilePath).catch(() => absoluteFilePath),
        ]);
        if (!isWithinRoot(rootRealPath, fileRealPath)) continue;

        const fileStat = await stat(fileRealPath).catch(() => null);
        if (!fileStat || !fileStat.isFile()) continue;

        const headers = new Headers();
        headers.set("Content-Type", getMimeType(fileRealPath));
        headers.set("Cache-Control", getCacheControl(pathname));
        headers.set("X-Content-Type-Options", "nosniff");
        headers.set("Content-Length", String(fileStat.size));

        if (request.method === "HEAD") {
          return new Response(null, { status: 200, headers });
        }

        const body = await readFile(fileRealPath);
        return new Response(body, { status: 200, headers });
      }

      if (matchedStaticLikePath) {
        return new Response("Not Found", { status: 404 });
      }
    }
  );
};
