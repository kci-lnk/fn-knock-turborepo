export type AcmeCertificateAuthority = "zerossl" | "letsencrypt";

export const DEFAULT_ACME_CERTIFICATE_AUTHORITY: AcmeCertificateAuthority =
  "zerossl";

export const normalizeAcmeCertificateAuthority = (
  value: string | null | undefined,
): AcmeCertificateAuthority => {
  if (value === "letsencrypt") return "letsencrypt";
  return DEFAULT_ACME_CERTIFICATE_AUTHORITY;
};

