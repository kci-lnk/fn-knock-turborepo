const WINDOWS_UNSAFE_FILENAME_CHARACTERS = /[<>:"/\\|?*\u0000-\u001f]/g;

export const acmeCertificateArchiveStem = (domain: string) => {
  const trimmed = domain.trim().replace(/\.+$/, "");
  const wildcardSafe = trimmed.startsWith("*.")
    ? `wildcard.${trimmed.slice(2)}`
    : trimmed;
  const portable = wildcardSafe
    .replace(WINDOWS_UNSAFE_FILENAME_CHARACTERS, "_")
    .replace(/^[ .]+|[ .]+$/g, "");

  return portable || "certificate";
};

export const acmeCertificateArchiveFilename = (domain: string) =>
  `${acmeCertificateArchiveStem(domain)}.zip`;
