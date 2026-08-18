export type ScannerPathDraftEntry = {
  id: number;
  value: string;
};

export type ScannerPathValidationError =
  "required" | "absolute" | "controlCharacters" | "duplicate";

export const normalizeScannerWhitelistPath = (value: string) => {
  const path = value.trim().split("?")[0]?.split("#")[0] ?? "";
  if (!path) return "";
  return path === "/" ? path : path.replace(/\/$/u, "") || "/";
};

export const validateScannerWhitelistEntries = (
  entries: ScannerPathDraftEntry[],
) => {
  const errors = new Map<number, ScannerPathValidationError>();
  const canonicalOwners = new Map<string, number>();
  for (const entry of entries) {
    const hasControlCharacter = Array.from(entry.value).some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint < 32 || (codePoint >= 127 && codePoint <= 159);
    });
    if (hasControlCharacter) {
      errors.set(entry.id, "controlCharacters");
      continue;
    }
    const value = entry.value.trim();
    if (!value) {
      errors.set(entry.id, "required");
      continue;
    }
    if (!value.startsWith("/")) {
      errors.set(entry.id, "absolute");
      continue;
    }
    const canonical = normalizeScannerWhitelistPath(value);
    const existingOwner = canonicalOwners.get(canonical);
    if (existingOwner !== undefined) {
      errors.set(entry.id, "duplicate");
      errors.set(existingOwner, "duplicate");
    } else {
      canonicalOwners.set(canonical, entry.id);
    }
  }
  return errors;
};
