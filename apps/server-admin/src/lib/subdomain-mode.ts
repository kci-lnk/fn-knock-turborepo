import type { AuthConfig } from "./go-backend";
import type { AppConfig, HostMapping } from "./redis";

const trimTrailingSlash = (value: string): string => value.replace(/\/+$/, "");

const resolveForwardedProto = (request: Request): string => {
  const forwarded = request.headers
    .get("x-forwarded-proto")
    ?.split(",")[0]
    ?.trim();
  if (forwarded === "http" || forwarded === "https") return forwarded;

  try {
    return new URL(request.url).protocol.replace(":", "") || "https";
  } catch {
    return "https";
  }
};

const resolveForwardedHost = (request: Request): string => {
  const forwarded = request.headers
    .get("x-forwarded-host")
    ?.split(",")[0]
    ?.trim();
  if (forwarded) return forwarded;

  try {
    return new URL(request.url).host;
  } catch {
    return request.headers.get("host")?.trim() || "";
  }
};

const parseInternalPortFromTarget = (target: string): number | null => {
  try {
    const parsed = new URL(target);
    const port = Number.parseInt(parsed.port, 10);
    if (Number.isFinite(port) && port > 0) {
      return port;
    }
    if (parsed.protocol === "https:") return 443;
    if (parsed.protocol === "http:") return 80;
  } catch {
    // ignore and fallback below
  }

  return null;
};

const resolveDefaultPublicGatewayPort = (): number | null => {
  const parsed = Number.parseInt(process.env.GO_REPROXY_PORT || "", 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return parsed;
};

const formatDerivedPublicAuthBaseUrl = (
  host: string,
  scheme: "http" | "https" = "https",
): string => {
  const normalizedHost = host.trim().toLowerCase();
  if (!normalizedHost) return "";

  const port = resolveDefaultPublicGatewayPort();
  if (!port) return `${scheme}://${normalizedHost}`;

  const isDefaultPort =
    (scheme === "https" && port === 443) || (scheme === "http" && port === 80);

  return isDefaultPort
    ? `${scheme}://${normalizedHost}`
    : `${scheme}://${normalizedHost}:${port}`;
};

const normalizeDomainName = (value: string | undefined | null): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/\.$/, "");

const isWildcardDomain = (value: string): boolean =>
  normalizeDomainName(value).startsWith("*.");

const stripWildcardPrefix = (value: string): string =>
  normalizeDomainName(value).replace(/^\*\./, "");

const uniqStrings = (values: string[]): string[] => {
  const seen = new Set<string>();
  const output: string[] = [];
  for (const value of values) {
    const normalized = value.trim().toLowerCase();
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    output.push(normalized);
  }
  return output;
};

const doesPatternCoverConcreteHost = (
  concreteHost: string,
  pattern: string,
): boolean => {
  const normalizedHost = normalizeDomainName(concreteHost);
  const normalizedPattern = normalizeDomainName(pattern);
  if (!normalizedHost || !normalizedPattern || isWildcardDomain(normalizedHost)) {
    return false;
  }

  if (!isWildcardDomain(normalizedPattern)) {
    return normalizedHost === normalizedPattern;
  }

  const suffix = stripWildcardPrefix(normalizedPattern);
  if (!suffix || !normalizedHost.endsWith(`.${suffix}`)) return false;

  const label = normalizedHost.slice(
    0,
    normalizedHost.length - suffix.length - 1,
  );
  return !!label && !label.includes(".");
};

const isRequirementCoveredByCertificateDomains = (
  requirement: string,
  certificateDomains: string[],
): boolean => {
  const normalizedRequirement = normalizeDomainName(requirement);
  if (!normalizedRequirement) return false;

  if (isWildcardDomain(normalizedRequirement)) {
    return certificateDomains.some(
      (domain) => normalizeDomainName(domain) === normalizedRequirement,
    );
  }

  return certificateDomains.some((domain) =>
    doesPatternCoverConcreteHost(normalizedRequirement, domain),
  );
};

export type SubdomainCertificateRecommendation = {
  mode: "wildcard_parent" | "single_host" | "manual";
  root_domain?: string;
  auth_host?: string;
  recommended_domains: string[];
  covered_hosts: string[];
  uncovered_hosts: string[];
  warnings: string[];
  can_autofill: boolean;
  summary: string;
};

export type SubdomainCertificateCoverage = {
  status: "ready" | "partial" | "missing";
  auth_host?: string;
  certificate_domains: string[];
  recommended_domains: string[];
  covered_recommended_domains: string[];
  uncovered_recommended_domains: string[];
  covered_hosts: string[];
  uncovered_hosts: string[];
  covers_auth_host: boolean;
  warnings: string[];
  summary: string;
};

export type SubdomainCertificateInventoryCoverage = {
  status: "ready" | "partial" | "missing";
  deployment_mode: "single_active" | "multi_sni";
  active_certificate_id?: string;
  fully_covering_certificate_ids: string[];
  partially_covering_certificate_ids: string[];
  combined_covering_certificate_ids: string[];
  suggested_certificate_id?: string;
  can_auto_activate: boolean;
  warnings: string[];
  summary: string;
};

export const getAuthHostMapping = (
  config: Pick<AppConfig, "host_mappings">,
): HostMapping | null => {
  const authMapping = config.host_mappings.find(
    (mapping) => mapping.service_role === "auth",
  );
  return authMapping || null;
};

export const buildSubdomainCertificateRecommendation = (
  config: Pick<AppConfig, "subdomain_mode" | "host_mappings">,
): SubdomainCertificateRecommendation => {
  const rootDomain =
    config.subdomain_mode?.root_domain?.trim().toLowerCase() || "";
  const authHost =
    getAuthHostMapping(config)?.host?.trim().toLowerCase() ||
    config.subdomain_mode?.auth_host?.trim().toLowerCase() ||
    "";
  const allHosts = uniqStrings(
    (config.host_mappings || []).map((mapping) => mapping.host || ""),
  );

  let mode: SubdomainCertificateRecommendation["mode"] = "manual";
  let summary = "尚未配置根域名或鉴权服务，暂时无法生成推荐证书域名。";
  const warnings: string[] = [];
  let recommendedDomains: string[] = [];

  if (rootDomain) {
    mode = "wildcard_parent";
    recommendedDomains = uniqStrings([rootDomain, `*.${rootDomain}`]);
    summary = `推荐申请 ${rootDomain} 与 *.${rootDomain}，用于覆盖根域名、鉴权服务和同一父域下的业务子域。`;

    if (
      authHost &&
      !isRequirementCoveredByCertificateDomains(authHost, recommendedDomains)
    ) {
      recommendedDomains = uniqStrings([...recommendedDomains, authHost]);
      warnings.push(
        `当前鉴权服务 ${authHost} 不在根域名 ${rootDomain} 下，已额外加入精确域名；请确认所选 DNS 服务商能够管理这些域名。`,
      );
    }
  } else if (authHost) {
    mode = "single_host";
    recommendedDomains = [authHost];
    summary = `尚未配置根域名，当前仅能推荐为鉴权服务 ${authHost} 申请单域名证书。`;
    warnings.push("如果后续要统一覆盖多个业务子域，建议先补充根域名后再申请 wildcard 证书。");
  } else {
    warnings.push("请先在子域名模式里配置根域名，或在 Host 映射中指定一条鉴权服务。");
  }

  if (!authHost) {
    warnings.push("尚未指定鉴权服务，当前推荐结果只基于根域名推导。");
  }

  const coveredHosts = allHosts.filter((host) =>
    isRequirementCoveredByCertificateDomains(host, recommendedDomains),
  );
  const uncoveredHosts = allHosts.filter(
    (host) => !isRequirementCoveredByCertificateDomains(host, recommendedDomains),
  );

  if (uncoveredHosts.length > 0 && recommendedDomains.length > 0) {
    warnings.push(
      `当前有 ${uncoveredHosts.length} 个 Host 映射不在推荐证书的覆盖范围内，如需对外暴露，仍需额外证书或调整域名规划。`,
    );
  }

  return {
    mode,
    root_domain: rootDomain || undefined,
    auth_host: authHost || undefined,
    recommended_domains: recommendedDomains,
    covered_hosts: coveredHosts,
    uncovered_hosts: uncoveredHosts,
    warnings,
    can_autofill: recommendedDomains.length > 0,
    summary,
  };
};

export const buildSubdomainCertificateCoverage = ({
  config,
  certificateDomains,
}: {
  config: Pick<AppConfig, "subdomain_mode" | "host_mappings">;
  certificateDomains?: string[] | null;
}): SubdomainCertificateCoverage => {
  const recommendation = buildSubdomainCertificateRecommendation(config);
  const currentCertificateDomains = uniqStrings(certificateDomains || []);
  const allHosts = uniqStrings(
    (config.host_mappings || []).map((mapping) => mapping.host || ""),
  );
  const coveredRecommendedDomains = recommendation.recommended_domains.filter(
    (domain) =>
      isRequirementCoveredByCertificateDomains(domain, currentCertificateDomains),
  );
  const uncoveredRecommendedDomains = recommendation.recommended_domains.filter(
    (domain) =>
      !isRequirementCoveredByCertificateDomains(
        domain,
        currentCertificateDomains,
      ),
  );
  const coveredHosts = allHosts.filter((host) =>
    isRequirementCoveredByCertificateDomains(host, currentCertificateDomains),
  );
  const uncoveredHosts = allHosts.filter(
    (host) =>
      !isRequirementCoveredByCertificateDomains(host, currentCertificateDomains),
  );
  const authHost = recommendation.auth_host;
  const coversAuthHost = authHost
    ? isRequirementCoveredByCertificateDomains(authHost, currentCertificateDomains)
    : false;

  let status: SubdomainCertificateCoverage["status"] = "missing";
  let summary = "当前未启用 SSL 证书，鉴权服务与业务子域尚未被 HTTPS 覆盖。";
  const warnings = [...recommendation.warnings];

  if (currentCertificateDomains.length === 0) {
    if (!recommendation.can_autofill) {
      summary = recommendation.summary;
    }
  } else if (
    uncoveredRecommendedDomains.length === 0 &&
    uncoveredHosts.length === 0
  ) {
    status = "ready";
    summary = "当前已部署证书覆盖了推荐域名，并匹配所有已配置 Host 映射。";
  } else if (coveredRecommendedDomains.length > 0 || coveredHosts.length > 0) {
    status = "partial";
    summary =
      "当前证书只覆盖了部分子域名模式所需域名，鉴权服务或部分业务 Host 仍可能出现证书不匹配。";
  } else {
    summary =
      "当前已部署证书与子域名模式不匹配，鉴权服务和业务 Host 仍未被正确覆盖。";
  }

  if (currentCertificateDomains.length > 0 && uncoveredRecommendedDomains.length > 0) {
    warnings.push(
      `当前证书还缺少 ${uncoveredRecommendedDomains.length} 个推荐域名覆盖项，建议重新申请或替换证书。`,
    );
  }

  if (currentCertificateDomains.length > 0 && authHost && !coversAuthHost) {
    warnings.push(`当前证书未覆盖鉴权服务 ${authHost}。`);
  }

  return {
    status,
    auth_host: authHost || undefined,
    certificate_domains: currentCertificateDomains,
    recommended_domains: recommendation.recommended_domains,
    covered_recommended_domains: coveredRecommendedDomains,
    uncovered_recommended_domains: uncoveredRecommendedDomains,
    covered_hosts: coveredHosts,
    uncovered_hosts: uncoveredHosts,
    covers_auth_host: coversAuthHost,
    warnings,
    summary,
  };
};

export const buildSubdomainCertificateInventoryCoverage = ({
  config,
  certificates,
  activeCertificateId,
  deploymentMode = "single_active",
}: {
  config: Pick<AppConfig, "subdomain_mode" | "host_mappings">;
  certificates: Array<{
    id: string;
    certificateDomains?: string[] | null;
  }>;
  activeCertificateId?: string | null;
  deploymentMode?: "single_active" | "multi_sni";
}): SubdomainCertificateInventoryCoverage => {
  const recommendation = buildSubdomainCertificateRecommendation(config);
  const allHosts = uniqStrings(
    (config.host_mappings || []).map((mapping) => mapping.host || ""),
  );
  const requirements = uniqStrings([
    ...recommendation.recommended_domains,
    ...allHosts,
  ]);

  const analyses = certificates.map((certificate) => {
    const normalizedDomains = uniqStrings(certificate.certificateDomains || []);
    const coverage = buildSubdomainCertificateCoverage({
      config,
      certificateDomains: normalizedDomains,
    });
    const coveredRequirements = requirements.filter((requirement) =>
      isRequirementCoveredByCertificateDomains(requirement, normalizedDomains),
    );

    return {
      id: certificate.id,
      coverage,
      coveredRequirements,
    };
  });

  const fullyCovering = analyses.filter(
    (item) => item.coverage.status === "ready",
  );
  const partiallyCovering = analyses.filter(
    (item) =>
      item.coverage.status !== "ready" && item.coveredRequirements.length > 0,
  );
  const activeAnalysis =
    analyses.find((item) => item.id === activeCertificateId) || null;

  const uncoveredRequirements = new Set(requirements);
  const combinedCoveringCertificateIds: string[] = [];
  const remaining = analyses.slice();

  while (uncoveredRequirements.size > 0 && remaining.length > 0) {
    let bestIndex = -1;
    let bestGain = 0;

    remaining.forEach((item, index) => {
      const gain = item.coveredRequirements.filter((requirement) =>
        uncoveredRequirements.has(requirement),
      ).length;
      if (gain > bestGain) {
        bestGain = gain;
        bestIndex = index;
      }
    });

    if (bestIndex === -1 || bestGain === 0) break;

    const [selected] = remaining.splice(bestIndex, 1);
    if (!selected) break;
    combinedCoveringCertificateIds.push(selected.id);
    selected.coveredRequirements.forEach((requirement) => {
      uncoveredRequirements.delete(requirement);
    });
  }

  const combinedReady =
    requirements.length > 0 && uncoveredRequirements.size === 0;

  let status: SubdomainCertificateInventoryCoverage["status"] = "missing";
  let summary = "证书库中还没有可用于子域名模式的证书。";
  const warnings: string[] = [];

  if (activeAnalysis?.coverage.status === "ready") {
    status = "ready";
    summary = "当前活动证书已经完整覆盖子域名模式所需域名。";
  } else if (fullyCovering.length === 1) {
    status = "ready";
    summary = "证书库中有 1 张证书可完整覆盖子域名模式，可以直接切换为活动证书。";
  } else if (fullyCovering.length > 1) {
    status = "ready";
    summary = `证书库中有 ${fullyCovering.length} 张证书各自都能完整覆盖当前子域名模式。`;
  } else if (combinedReady && deploymentMode === "multi_sni") {
    status = "ready";
    summary =
      combinedCoveringCertificateIds.length > 1
        ? "证书库组合后已经具备完整覆盖能力。"
        : "证书库中已有可覆盖当前子域名模式的候选证书。";
  } else if (combinedReady) {
    status = "partial";
    summary =
      "证书库组合后已经可以覆盖当前子域名模式，但当前网关仍是单活动证书模式，暂时不能同时生效。";
  } else if (partiallyCovering.length > 0) {
    status = "partial";
    summary =
      "证书库里已有部分候选证书，但还不能完整覆盖鉴权服务和全部 Host 映射。";
  } else if (recommendation.can_autofill) {
    summary = "当前还没有证书能够覆盖子域名模式推荐域名。";
  } else {
    summary = recommendation.summary;
  }

  if (
    combinedReady &&
    combinedCoveringCertificateIds.length > 1 &&
    deploymentMode !== "multi_sni"
  ) {
    warnings.push(
      "当前证书库需要多张证书联合覆盖，但网关仍处于单活动证书模式，暂时无法一次性全部生效。",
    );
  }

  if (
    activeAnalysis &&
    activeAnalysis.coverage.status !== "ready" &&
    fullyCovering.length === 1
  ) {
    warnings.push("当前活动证书与子域名模式不完全匹配，建议切换到推荐证书。");
  }

  if (
    !activeAnalysis &&
    fullyCovering.length === 0 &&
    combinedCoveringCertificateIds.length > 1
  ) {
    warnings.push("现有证书库更适合后续多证书/SNI 部署。");
  }

  return {
    status,
    deployment_mode: deploymentMode,
    active_certificate_id: activeAnalysis?.id,
    fully_covering_certificate_ids: fullyCovering.map((item) => item.id),
    partially_covering_certificate_ids: partiallyCovering.map((item) => item.id),
    combined_covering_certificate_ids: combinedCoveringCertificateIds,
    suggested_certificate_id:
      activeAnalysis?.coverage.status === "ready" || fullyCovering.length !== 1
        ? undefined
        : fullyCovering[0]?.id,
    can_auto_activate:
      activeAnalysis?.coverage.status !== "ready" && fullyCovering.length === 1,
    warnings,
    summary,
  };
};

export const resolvePublicAuthBaseUrl = (
  config: Pick<AppConfig, "subdomain_mode" | "host_mappings">,
): string => {
  const explicit = trimTrailingSlash(
    config.subdomain_mode?.public_auth_base_url?.trim() || "",
  );
  if (explicit) return explicit;

  const authMapping = getAuthHostMapping(config);
  if (authMapping?.host) {
    return formatDerivedPublicAuthBaseUrl(authMapping.host);
  }

  const authHost = config.subdomain_mode?.auth_host?.trim();
  if (authHost) return formatDerivedPublicAuthBaseUrl(authHost);

  return "";
};

export const resolveCookieDomain = (
  config?: Pick<AppConfig, "subdomain_mode"> | null,
): string | undefined => {
  const fromConfig = config?.subdomain_mode?.cookie_domain?.trim();
  if (fromConfig) return fromConfig;

  const fromEnv = process.env.SESSION_COOKIE_DOMAIN?.trim();
  return fromEnv || undefined;
};

export const buildGatewayAuthConfig = (
  config: Pick<AppConfig, "subdomain_mode" | "host_mappings">,
): AuthConfig => {
  const defaultAuthPort = Number.parseInt(process.env.AUTH_PORT || "7997", 10);
  const authMapping = getAuthHostMapping(config);
  const authTarget =
    authMapping?.target?.trim() || config.subdomain_mode?.auth_target?.trim() || "";
  const authPort =
    parseInternalPortFromTarget(authTarget) ??
    (Number.isFinite(defaultAuthPort) && defaultAuthPort > 0
      ? defaultAuthPort
      : 7997);
  const publicAuthBaseUrl = resolvePublicAuthBaseUrl(config);
  const authHost =
    authMapping?.host?.trim() || config.subdomain_mode?.auth_host?.trim() || "";

  return {
    auth_port: authPort,
    auth_url: "/api/auth/verify",
    login_url: "/#/login",
    logout_url: "/api/auth/logout",
    preflight_url: "/api/auth/preflight",
    public_auth_base_url: publicAuthBaseUrl || undefined,
    auth_host: authHost || undefined,
  };
};

export const resolveSafeRedirectUri = ({
  config,
  request,
  redirectUri,
}: {
  config: Pick<AppConfig, "subdomain_mode" | "host_mappings">;
  request: Request;
  redirectUri?: string | null;
}): string | null => {
  const raw = redirectUri?.trim();
  if (!raw) return null;

  const requestProto = resolveForwardedProto(request);
  const requestHost = resolveForwardedHost(request);

  if (raw.startsWith("/")) {
    return raw;
  }

  let target: URL;
  try {
    target = new URL(raw);
  } catch {
    return null;
  }

  if (target.protocol !== "http:" && target.protocol !== "https:") {
    return null;
  }

  const currentOrigin = requestHost ? `${requestProto}://${requestHost}` : "";
  if (currentOrigin) {
    try {
      if (target.origin === new URL(currentOrigin).origin) {
        return target.toString();
      }
    } catch {
      // ignore and continue with configured rules
    }
  }

  const rootDomain =
    config.subdomain_mode?.root_domain?.trim().toLowerCase() || "";
  if (rootDomain) {
    const hostname = target.hostname.toLowerCase();
    if (hostname === rootDomain || hostname.endsWith(`.${rootDomain}`)) {
      return target.toString();
    }
  }

  const authBaseUrl = resolvePublicAuthBaseUrl(config);
  if (authBaseUrl) {
    try {
      const authOrigin = new URL(authBaseUrl).origin;
      if (target.origin === authOrigin) {
        return target.toString();
      }
    } catch {
      // ignore invalid configured auth url
    }
  }

  return null;
};
