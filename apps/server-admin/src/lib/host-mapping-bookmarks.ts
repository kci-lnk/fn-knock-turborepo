import { isAuthServiceMapping } from "./auth-service";
import { resolveHostMappingDisplayTitle } from "./host-mapping-metadata";
import type { HostMapping } from "./redis";

const DEFAULT_ACCESS_ENTRY_PORT = "7999";

export type BookmarkScheme = "http" | "https";

export interface HostMappingBookmarksDocumentOptions {
  mappings: HostMapping[];
  scheme?: BookmarkScheme;
  accessEntryPort?: number | string | null;
  folderTitle?: string;
  exportedAt?: Date;
}

const escapeHtml = (value: string): string =>
  value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

const normalizeHost = (value: string): string =>
  value.trim().toLowerCase().replace(/\.+$/, "");

const normalizeFilenamePart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9.-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");

const resolveAccessEntryPort = (
  accessEntryPort?: number | string | null,
): string => {
  const normalized = String(accessEntryPort ?? "").trim();
  if (normalized) return normalized;

  const envPort = String(process.env.GO_REPROXY_PORT ?? "").trim();
  return envPort || DEFAULT_ACCESS_ENTRY_PORT;
};

const resolveBookmarkTitle = (mapping: HostMapping): string =>
  resolveHostMappingDisplayTitle(mapping) || mapping.host.trim();

const buildBookmarkUrl = ({
  host,
  scheme,
  accessEntryPort,
}: {
  host: string;
  scheme: BookmarkScheme;
  accessEntryPort?: number | string | null;
}): string => {
  const normalizedHost = normalizeHost(host);
  if (!normalizedHost) return "";

  const port = resolveAccessEntryPort(accessEntryPort);
  const parsedPort = Number.parseInt(port, 10);
  const isDefaultPort =
    Number.isFinite(parsedPort) &&
    ((scheme === "http" && parsedPort === 80) ||
      (scheme === "https" && parsedPort === 443));
  const portSuffix = !port || isDefaultPort ? "" : `:${port}`;

  return `${scheme}://${normalizedHost}${portSuffix}/`;
};

const buildBookmarkTimestamp = (date: Date): string =>
  `${Math.floor(date.getTime() / 1000)}`;

export const buildHostMappingsBookmarkFilename = (
  rootDomain?: string | null,
): string => {
  const normalizedRootDomain = normalizeFilenamePart(String(rootDomain ?? ""));
  return normalizedRootDomain
    ? `fn-knock-bookmarks-${normalizedRootDomain}.html`
    : "fn-knock-bookmarks.html";
};

export const buildHostMappingsBookmarksDocument = ({
  mappings,
  scheme = "https",
  accessEntryPort,
  folderTitle,
  exportedAt = new Date(),
}: HostMappingBookmarksDocumentOptions): string => {
  const addDate = buildBookmarkTimestamp(exportedAt);
  const resolvedFolderTitle = folderTitle?.trim() || "fn-knock 子域映射";
  const bookmarkLines = mappings
    .filter((mapping) => !isAuthServiceMapping(mapping))
    .map((mapping) => {
      const href = buildBookmarkUrl({
        host: mapping.host,
        scheme,
        accessEntryPort,
      });
      if (!href) return "";

      const title = resolveBookmarkTitle(mapping);
      return `    <DT><A HREF="${escapeHtml(href)}" ADD_DATE="${addDate}">${escapeHtml(title)}</A>`;
    })
    .filter(Boolean);

  return [
    "<!DOCTYPE NETSCAPE-Bookmark-file-1>",
    "<!-- This is an automatically generated file.",
    "     It will be read and overwritten.",
    "     DO NOT EDIT! -->",
    '<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">',
    "<TITLE>Bookmarks</TITLE>",
    "<H1>Bookmarks</H1>",
    "<DL><p>",
    `  <DT><H3 ADD_DATE="${addDate}" LAST_MODIFIED="${addDate}">${escapeHtml(resolvedFolderTitle)}</H3>`,
    "  <DL><p>",
    ...bookmarkLines,
    "  </DL><p>",
    "</DL><p>",
    "",
  ].join("\n");
};
