import { parseHostPort } from "@admin-shared/utils/parseHostPort";
import type { HostMapping } from "@/types";

// Host/port parsing alone accepts punctuation and empty DNS labels. Keep
// local names, IDNs, wildcard subdomains and IP literals supported as well.
export const isValidBatchMappingHost = (host: string): boolean => {
  const parsed = parseHostPort(`${host}:80`);
  if (!parsed) return false;
  if (parsed.isIPv6) return true;
  try {
    const name = host.startsWith("*.") ? host.slice(2) : host;
    if (name.includes("%")) return false;
    const ascii = new URL(`http://${name}`).hostname;
    return (
      ascii.length <= 253 &&
      ascii
        .split(".")
        .every((label) =>
          /^[a-z\d_](?:[a-z\d_-]{0,61}[a-z\d_])?$/iu.test(label),
        )
    );
  } catch {
    return false;
  }
};

// Metadata refreshes and object key ordering do not change editable config.
// Preserve array order, since mapping order and ordered settings are meaningful.
export const batchMappingSignature = (mappings: HostMapping[]) =>
  JSON.stringify(
    mappings.map((mapping) => ({ ...mapping, title: "", favicon: "" })),
    (_key, value: unknown) =>
      value && typeof value === "object" && !Array.isArray(value)
        ? Object.fromEntries(
            Object.entries(value).sort(([a], [b]) => a.localeCompare(b)),
          )
        : value,
  );
