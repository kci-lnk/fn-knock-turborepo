type TranslationParams = Record<
  string,
  string | number | boolean | null | undefined
>;

type AcmeRouteTranslate = (key: string, params?: TranslationParams) => string;

export type AcmeLogAnalysis = {
  reason:
    | "dns_credentials_invalid"
    | "dns_credentials_invalid_email"
    | "dns_api_rate_limited"
    | "acme_frequency_limited"
    | "unknown";
  provider?: string;
  message: string;
  evidence?: string[];
};

const pickEvidence = (
  logs: string[],
  match: (line: string) => boolean,
  max: number = 3,
) => {
  const hits: string[] = [];
  for (let i = logs.length - 1; i >= 0; i--) {
    const line = logs[i];
    if (!line) continue;
    if (!match(line)) continue;
    hits.push(line);
    if (hits.length >= max) break;
  }
  return hits.length ? hits.reverse() : undefined;
};

export const analyzeAcmeLogs = (
  job: { provider?: string | null },
  logs: string[],
  translate: AcmeRouteTranslate,
): AcmeLogAnalysis | null => {
  if (!logs.length) return null;
  const provider = job.provider || undefined;

  const has = (re: RegExp) => logs.some((line) => re.test(line));

  const isCloudflare =
    provider === "dns_cf" || has(/\bCloudflare\b/i) || has(/\bX-Auth-Key\b/i);
  if (isCloudflare) {
    const invalidKey =
      has(/Invalid format for X-Auth-Key header/i) || has(/"code"\s*:\s*6103/i);
    if (invalidKey) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: translate("cloudflareInvalidKey"),
        evidence: pickEvidence(
          logs,
          (line) => /X-Auth-Key/i.test(line) || /"code"\s*:\s*6103/i.test(line),
        ),
      };
    }

    const invalidEmail = has(/Invalid format for X-Auth-Email header/i);
    if (invalidEmail) {
      return {
        reason: "dns_credentials_invalid_email",
        provider: "dns_cf",
        message: translate("cloudflareInvalidEmail"),
        evidence: pickEvidence(logs, (line) => /X-Auth-Email/i.test(line)),
      };
    }

    const invalidHeaders =
      has(/Invalid request headers/i) || has(/"code"\s*:\s*6003/i);
    if (invalidHeaders) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: translate("cloudflareInvalidHeaders"),
        evidence: pickEvidence(
          logs,
          (line) =>
            /Invalid request headers/i.test(line) ||
            /"code"\s*:\s*6003/i.test(line),
        ),
      };
    }
  }

  const retryAfterLine = [...logs]
    .reverse()
    .find((line) => /retryafter\s*=\s*\d+/i.test(line));
  if (retryAfterLine && /will not retry|too large/i.test(retryAfterLine)) {
    const m = retryAfterLine.match(/retryafter\s*=\s*(\d+)/i);
    const seconds = m ? Number(m[1]) : NaN;
    const isTooLarge = Number.isFinite(seconds) && seconds > 600;
    if (isTooLarge) {
      return {
        reason: "acme_frequency_limited",
        provider,
        message: translate("acmeFrequencyLimited", { seconds }),
        evidence: pickEvidence(
          logs,
          (line) =>
            /retryafter\s*=\s*\d+/i.test(line) ||
            /will not retry|too large/i.test(line),
        ),
      };
    }
  }

  const rateLimited = has(/rate limit|too many requests|429/i);
  if (rateLimited) {
    return {
      reason: "dns_api_rate_limited",
      provider,
      message: translate("dnsApiRateLimited"),
      evidence: pickEvidence(logs, (line) =>
        /rate limit|too many requests|429/i.test(line),
      ),
    };
  }

  const failure = has(/failed|invalid/i);
  if (failure) {
    return {
      reason: "unknown",
      provider,
      message: translate("logUnknownFailure"),
      evidence: pickEvidence(logs, (line) => /failed|invalid/i.test(line)),
    };
  }

  return null;
};
