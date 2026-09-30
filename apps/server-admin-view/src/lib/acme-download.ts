// eslint-disable-next-line no-control-regex -- control characters are intentionally rejected from archive filenames.
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

export const acmeCertificateOutputPaths = (
  directory: string,
  domain: string,
) => {
  const trimmed = directory.trim();
  const windows = /^[a-z]:[\\/]/i.test(trimmed) || trimmed.startsWith("\\\\");
  const separator = windows ? "\\" : "/";
  const base = windows
    ? trimmed.replace(/\//g, "\\").replace(/\\+$/, "")
    : trimmed.replace(/\/+$/, "");
  const stem = acmeCertificateArchiveStem(domain.trim().toLowerCase());
  return {
    certificatePath: `${base}${separator}${stem}.cert.pem`,
    privateKeyPath: `${base}${separator}${stem}.key.pem`,
  };
};
