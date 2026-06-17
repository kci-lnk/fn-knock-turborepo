import { Elysia } from "elysia";
import { stat, readFile, realpath } from "node:fs/promises";
import { extname, isAbsolute, relative, resolve, sep } from "node:path";
import { brotliCompress, gzip } from "node:zlib";
import { promisify } from "node:util";

type StaticFilesPluginOptions = {
  root: string;
  mountPrefixes?: string[];
  excludePaths?: string[];
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

const gzipAsync = promisify(gzip);
const brotliCompressAsync = promisify(brotliCompress);

const COMPRESSIBLE_EXTENSIONS = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".map",
  ".mjs",
  ".svg",
  ".txt",
]);

const MIN_COMPRESS_SIZE = 1024;

const normalizePrefix = (value: string): string => {
  const trimmed = value.trim();
  if (trimmed.length === 0 || trimmed === "/") return "/";
  const withSlash = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  return withSlash.endsWith("/") ? withSlash.slice(0, -1) : withSlash;
};

const normalizePath = (value: string): string => {
  const trimmed = value.trim();
  if (trimmed.length === 0 || trimmed === "/") return "/";
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
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

const isFingerprintedAssetPath = (pathname: string): boolean => {
  if (!pathname.includes("/assets/")) return false;
  const fileName = pathname.split("/").pop() ?? "";
  return /-[A-Za-z0-9_-]{7,}\.[^./]+$/.test(fileName);
};

const getCacheControl = (pathname: string): string => {
  // Fingerprinted bundles can be cached aggressively.
  if (isFingerprintedAssetPath(pathname)) return "public, max-age=31536000, immutable";
  return "public, max-age=300";
};

const appendVary = (headers: Headers, value: string) => {
  const existing = headers.get("Vary");
  if (!existing) {
    headers.set("Vary", value);
    return;
  }

  const values = existing.split(",").map((item) => item.trim().toLowerCase());
  if (!values.includes(value.toLowerCase())) {
    headers.set("Vary", `${existing}, ${value}`);
  }
};

type EncodingPreference = {
  name: "br" | "gzip";
  q: number;
  order: number;
};

const parseEncodingPreference = (
  value: string,
  order: number,
): EncodingPreference | null => {
  const [rawName, ...params] = value.split(";").map((item) => item.trim());
  const name = rawName?.toLowerCase();
  if (name !== "br" && name !== "gzip") return null;

  let q = 1;
  for (const param of params) {
    const [key, rawValue] = param.split("=").map((item) => item.trim());
    if (key?.toLowerCase() !== "q") continue;
    const nextQ = Number(rawValue);
    if (Number.isFinite(nextQ)) q = nextQ;
  }

  if (q <= 0) return null;
  return { name, q, order };
};

const getPreferredEncoding = (request: Request): "br" | "gzip" | null => {
  const acceptEncoding = request.headers.get("accept-encoding") ?? "";
  const encodings = acceptEncoding
    .split(",")
    .map((item, index) => parseEncodingPreference(item, index))
    .filter((item): item is EncodingPreference => Boolean(item))
    .sort((left, right) => {
      if (right.q !== left.q) return right.q - left.q;
      if (left.name !== right.name) return left.name === "br" ? -1 : 1;
      return left.order - right.order;
    });

  return encodings[0]?.name ?? null;
};

const shouldCompress = (pathname: string, body: Buffer) => {
  if (body.length < MIN_COMPRESS_SIZE) return false;
  return COMPRESSIBLE_EXTENSIONS.has(extname(pathname).toLowerCase());
};

const toArrayBuffer = (buffer: Buffer): ArrayBuffer => {
  const arrayBuffer = new ArrayBuffer(buffer.byteLength);
  new Uint8Array(arrayBuffer).set(buffer);
  return arrayBuffer;
};

export const createMaybeCompressedResponse = async ({
  body,
  headers,
  pathname,
  request,
  status = 200,
  head = false,
}: {
  body: Buffer | string;
  headers: Headers;
  pathname: string;
  request: Request;
  status?: number;
  head?: boolean;
}) => {
  const rawBody = Buffer.isBuffer(body) ? body : Buffer.from(body);
  let responseBody = rawBody;

  if (shouldCompress(pathname, rawBody)) {
    appendVary(headers, "Accept-Encoding");
    const encoding = getPreferredEncoding(request);
    if (encoding === "br") {
      responseBody = await brotliCompressAsync(rawBody);
      headers.set("Content-Encoding", "br");
    } else if (encoding === "gzip") {
      responseBody = await gzipAsync(rawBody);
      headers.set("Content-Encoding", "gzip");
    }
  }

  headers.set("Content-Length", String(responseBody.length));
  return new Response(head ? null : toArrayBuffer(responseBody), { status, headers });
};

export const createStaticFilesPlugin = ({
  root,
  mountPrefixes = ["/"],
  excludePaths = [],
  denyDotFiles = true,
}: StaticFilesPluginOptions) => {
  const absoluteRoot = resolve(root);
  const prefixes = mountPrefixes.map(normalizePrefix);
  const excludedPathSet = new Set(excludePaths.map(normalizePath));
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
      if (excludedPathSet.has(pathname) || excludedPathSet.has(rawPathname)) return;
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

        const body = await readFile(fileRealPath);
        return createMaybeCompressedResponse({
          body,
          headers,
          pathname,
          request,
          head: request.method === "HEAD",
        });
      }

      if (matchedStaticLikePath) {
        return new Response("Not Found", { status: 404 });
      }
    }
  );
};
