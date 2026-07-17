export type CloudflaredLogAnalysis = {
  reason: "origin_tls_hostname_mismatch";
  requestedHost: string;
  certificateHosts: string[];
  originUrl?: string;
  originHost?: string;
  evidence: string;
};

const ORIGIN_TLS_HOSTNAME_MISMATCH_REGEX =
  /tls:\s*failed to verify certificate:\s*x509:\s*certificate is valid for\s+(.+),\s*not\s+([^\s"]+)/i;
const DESTINATION_URL_REGEX = /\bdest=(https?:\/\/[^\s"]+)/i;

export const analyzeCloudflaredLogs = (
  lines: string[],
): CloudflaredLogAnalysis | null => {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]?.trim();
    if (!line) continue;

    const mismatchMatch = line.match(ORIGIN_TLS_HOSTNAME_MISMATCH_REGEX);
    if (!mismatchMatch) continue;

    const certificateHosts =
      mismatchMatch[1]
        ?.split(",")
        .map((item) => item.trim())
        .filter(Boolean) ?? [];
    const requestedHost = mismatchMatch[2]?.trim();
    if (!certificateHosts.length || !requestedHost) continue;

    const originUrl = line.match(DESTINATION_URL_REGEX)?.[1];
    let originHost: string | undefined;
    if (originUrl) {
      try {
        originHost = new URL(originUrl).hostname;
      } catch {
        originHost = undefined;
      }
    }

    return {
      reason: "origin_tls_hostname_mismatch",
      requestedHost,
      certificateHosts,
      originUrl,
      originHost,
      evidence: line,
    };
  }
  return null;
};
