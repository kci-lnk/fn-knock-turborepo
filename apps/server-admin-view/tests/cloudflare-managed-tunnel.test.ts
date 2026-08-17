import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const readOptimizationUi = () =>
  [
    "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    "../src/views/tunnel/cloudflare/CloudflareOptimizationOverview.vue",
    "../src/views/tunnel/cloudflare/CloudflareOptimizationDomains.vue",
    "../src/views/tunnel/cloudflare/CloudflareOptimizationTechnicalStatus.vue",
    "../src/views/tunnel/cloudflare/CloudflareOptimizationSourceSettings.vue",
    "../src/views/tunnel/cloudflare/CloudflareOptimizationScanResults.vue",
    "../src/views/tunnel/cloudflare/useCloudflareOptimizationCardPresentation.ts",
  ]
    .map(readSource)
    .join("\n");

const readManagedUi = () =>
  [
    "../src/views/tunnel/cloudflare/CloudflareManagedTunnelCard.vue",
    "../src/views/tunnel/cloudflare/CloudflareReconcilePlan.vue",
    "../src/views/tunnel/cloudflare/cloudflareManagedPresentation.ts",
  ]
    .map(readSource)
    .join("\n");

type ContractSchema = {
  enum?: string[];
  properties?: Record<string, ContractSchema>;
  required?: string[];
  writeOnly?: boolean;
};

const readContract = () =>
  JSON.parse(readSource("../../../packages/api-contract/openapi.json")) as {
    components: { schemas: Record<string, ContractSchema> };
  };

describe("managed Cloudflare Tunnel", () => {
  it("keeps runtime, managed reconciliation, and optimization lifecycles isolated", () => {
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    const presentation = readSource(
      "../src/views/tunnel/cloudflare/cloudflareOptimizationPresentation.ts",
    );
    for (const composable of [
      "useCloudflaredRuntime",
      "useCloudflareManagedTunnel",
      "useCloudflareOptimization",
    ]) {
      assert.match(controller, new RegExp(composable, "u"));
    }
    assert.doesNotMatch(controller, /CloudflaredAPI/u);
    assert.doesNotMatch(presentation, /useCloudflareTunnelController/u);
    assert.match(presentation, /CloudflareTranslate/u);
  });

  it("exposes the credential, reconcile, scan, apply, and fallback contracts", () => {
    const source = readSource("../src/lib/api/tunnel.ts");
    for (const route of [
      "/cloudflared/cloudflare/credential",
      "/cloudflared/cloudflare/state",
      "/cloudflared/reconcile/preview",
      "/cloudflared/reconcile/apply",
      "/cloudflared/reconcile/jobs/",
      "/cloudflared/optimization/scans",
      "/cloudflared/optimization/settings",
      "/cloudflared/optimization/domains/",
      "/cloudflared/optimization/apply",
      "/cloudflared/optimization/fallback",
    ]) {
      assert.match(
        source,
        new RegExp(route.replaceAll("/", String.raw`\/`), "u"),
      );
    }
  });

  it("runs reconcile apply as an idempotent polled job", () => {
    const api = readSource("../src/lib/api/tunnel.ts");
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareManagedTunnel.ts",
    );
    assert.match(api, /getReconcileJob\(/u);
    assert.match(api, /getReconcileJobByPlan\(/u);
    assert.match(api, /getActiveReconcileJob\(/u);
    assert.match(controller, /pollReconcileJob/u);
    assert.match(controller, /getReconcileJobByPlan\(planId\)/u);
    assert.match(controller, /recoverActiveReconcileJob/u);
    assert.match(controller, /\["queued", "running"\]/u);
    assert.match(controller, /reconcilePollSequence/u);
    assert.doesNotMatch(
      controller,
      /managedState\.value\s*=\s*await CloudflaredAPI\.applyReconcile/u,
    );
  });

  it("keeps Tunnel and API tokens write-only in the frontend contract", () => {
    const apiSource = readSource("../src/lib/api/tunnel.ts");
    assert.match(apiSource, /\["CloudflaredConfigData"\]/u);
    assert.match(apiSource, /\["CloudflareCredentialBodyData"\]/u);
    const configContract =
      readContract().components.schemas.CloudflaredConfigData;
    assert.ok(configContract.required?.includes("apiTokenConfigured"));
    assert.ok(configContract.required?.includes("tunnelTokenConfigured"));
    assert.equal(configContract.properties?.token, undefined);
    const schemas = readContract().components.schemas;
    assert.equal(
      schemas.CloudflaredConfigUpdateData.properties?.token?.writeOnly,
      true,
    );
    assert.equal(
      schemas.CloudflareCredentialBodyData.properties?.apiToken?.writeOnly,
      true,
    );

    const runtime = readSource(
      "../src/views/tunnel/cloudflare/useCloudflaredRuntime.ts",
    );
    const managed = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareManagedTunnel.ts",
    );
    assert.match(runtime, /token\.value = ""/u);
    assert.match(managed, /apiToken\.value = ""/u);
    assert.doesNotMatch(runtime, /token\.value\s*=\s*config\.token/u);
  });

  it("keeps automation, takeover preview, capability probe, and fallback visible", () => {
    const page = readSource("../src/views/tunnel/CloudflareTunnel.vue");
    assert.match(page, /CloudflareApiConnectionCard/u);
    assert.match(page, /CloudflareManagedTunnelCard/u);
    assert.match(page, /CloudflareOptimizationCard/u);
    assert.match(page, /CloudflareManualConfigCard/u);
    assert.ok(
      page.indexOf("admin.cloudflareTunnel.runtimeStatus") <
        page.indexOf("<CloudflareApiConnectionCard"),
    );
    assert.match(page, /:configured="false"/u);

    const managed = readManagedUi();
    assert.match(managed, /ConfigCollapsibleCard/u);
    assert.match(managed, /reconcilePlan\.operations/u);
    assert.match(managed, /toggleTakeover/u);
    assert.match(managed, /planWarningLabel/u);
    assert.match(managed, /warningCodes/u);
    assert.match(managed, /conflictMessageLabel/u);
    assert.match(managed, /conflict\.details\.records/u);
    assert.match(managed, /dnsOwnerLabel/u);
    assert.match(managed, /reconcileAttentionToken/u);
    assert.match(managed, /reconcileJob\.progress/u);
    assert.match(
      managed,
      /isPreviewingReconcile\s*\|\|\s*isApplyingReconcile/u,
    );
    assert.match(managed, /managedOperationTargetLabel/u);
    assert.match(managed, /keepDeleted/u);
    assert.match(managed, /toLocaleString\(locale\)/u);
    assert.doesNotMatch(managed, /text-muted-foreground">Tunnel</u);

    const managedController = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareManagedTunnel.ts",
    );
    assert.match(managedController, /apiAuthenticationFailed/u);
    assert.match(managedController, /code === 10_000/u);

    const optimization = readOptimizationUi();
    const optimizationPresentation = readSource(
      "../src/views/tunnel/cloudflare/cloudflareOptimizationPresentation.ts",
    );
    assert.match(optimization, /ConfigCollapsibleCard/u);
    assert.match(optimization, /allCandidates/u);
    assert.match(optimization, /capabilityProbe/u);
    assert.match(optimization, /fallbackOptimization/u);
    assert.match(optimization, /recommendedIp/u);
    assert.match(optimization, /candidateSources/u);
    assert.match(optimization, /businessColo \|\| candidate\.colo/u);
    assert.match(optimization, /optimizationCustomHostnames/u);
    assert.match(optimization, /sourceWarningLabel/u);
    assert.match(optimization, /domainMessageLabel/u);
    assert.match(optimization, /preserveExistingDns/u);
    assert.match(optimization, /prepareOptimizationConflictResolution/u);
    assert.match(optimization, /domain\.managementMode === ['"]external['"]/u);
    assert.match(optimization, /domain\.cleanupPending/u);
    assert.match(optimizationPresentation, /toLocaleString\(locale\)/u);
    assert.doesNotMatch(optimization, />Beta</u);
    assert.doesNotMatch(optimization, /<TableHead>IPv4/u);

    const connection = readSource(
      "../src/views/tunnel/cloudflare/CloudflareApiConnectionCard.vue",
    );
    assert.match(connection, /ConfigCollapsibleCard/u);
    assert.match(connection, /managed\.apiTokenLabel/u);
    assert.doesNotMatch(connection, />Cloudflare API Token</u);
    assert.doesNotMatch(connection, /permissionsTitle|CheckCircle2/u);

    const manual = readSource(
      "../src/views/tunnel/cloudflare/CloudflareManualConfigCard.vue",
    );
    assert.match(manual, /:configured="true"/u);
    assert.match(manual, /manual\.tunnelTokenLabel/u);
    assert.doesNotMatch(manual, />Tunnel Token</u);

    for (const source of [page, connection, managed, optimization, manual]) {
      assert.match(source, /#actions="\{ collapse \}"/u);
      assert.match(source, /@click="collapse"/u);
    }
  });

  it("does not animate button geometry when Cloudflare actions enter loading state", () => {
    const button = readSource(
      "../../../packages/ui-vue/src/components/ui/button/index.ts",
    );
    assert.match(button, /transition-colors/u);
    assert.doesNotMatch(button, /transition-all/u);
  });

  it("keeps third-party candidate hostnames DNS-only and provenance visible", () => {
    const backend = [
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/api.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/scheduler.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/resolvers.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/settings.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/probes.rs",
      ),
    ].join("\n");
    assert.match(backend, /cloudflare-dns\.com\/dns-query/u);
    assert.match(backend, /dns\.google\/dns-query/u);
    assert.match(backend, /doh\.pub\/dns-query/u);
    assert.match(backend, /dns\.alidns\.com\/dns-query/u);
    assert.match(backend, /application\/dns-message/u);
    assert.match(backend, /resolve_to_addrs/u);
    assert.match(backend, /\.no_proxy\(\)/u);
    assert.match(backend, /verified-multi-doh-fallback-v1/u);
    assert.match(backend, /candidate_ip_is_cloudflare/u);
    assert.match(backend, /source_hostnames/u);
    assert.match(backend, /business_validated/u);
    assert.match(backend, /bounded_cf_ray/u);
    assert.doesNotMatch(backend, /content:\s*source\.hostname/u);
    assert.doesNotMatch(backend, /www\.fbi\.gov/u);

    const managedBackend = readSource(
      "../../server-admin-rs/src/tunnels/cloudflared/managed.rs",
    );
    assert.match(managedBackend, /"warningCodes"/u);
    assert.match(managedBackend, /"messageCode": "unownedIngress"/u);
    assert.match(backend, /plan_warning_codes/u);
    assert.match(backend, /"messageCode": "unownedCustomHostname"/u);
    assert.match(backend, /"messageCode": "customHostnameQuotaExhausted"/u);
    assert.match(backend, /OPTIMIZATION_DOMAIN_SETTINGS_KEY/u);
    assert.match(backend, /configured_optimization_hosts/u);
    assert.match(backend, /relinquish_optimization_host/u);
    assert.match(backend, /reconcile_optimization_host_membership/u);
    assert.match(backend, /multipleExactDnsConflict/u);
    assert.match(backend, /multipleOptimizationDnsConflict/u);

    const resolverDiagnostics = readSource(
      "../src/views/tunnel/cloudflare/CloudflareResolverDiagnostics.vue",
    );
    const resolverPresentation = readSource(
      "../src/views/tunnel/cloudflare/cloudflareOptimizationPresentation.ts",
    );
    assert.match(resolverPresentation, /resolverPathAvailable/u);
    assert.match(resolverPresentation, /resolverPathOfficialRanges/u);
    assert.match(resolverPresentation, /resolverPathCurrentCandidate/u);
    assert.match(resolverPresentation, /resolverPathPreferredIp/u);
    assert.match(resolverDiagnostics, /lastErrorCode/u);

    for (const locale of ["zh-CN", "zh-Hant", "en", "ja-JP", "ko-KR"]) {
      const messages = readSource(
        `../../../packages/i18n/src/messages/admin/${locale}.ts`,
      );
      assert.doesNotMatch(messages, /us-fbi/u);
      assert.match(messages, /planWarnings/u);
      assert.match(messages, /candidateDiscoveryOnly/u);
      assert.match(messages, /conflictMessages/u);
      assert.match(messages, /domainMessages/u);
      assert.match(messages, /domainActions/u);
      assert.match(messages, /keepExternal/u);
      assert.match(messages, /externalCleanupPending/u);
      assert.match(messages, /multipleExactDnsConflict/u);
      assert.match(messages, /ownerKinds/u);
      assert.match(messages, /resolveFailed/u);
      assert.match(messages, /resolverDiagnosticsTitle/u);
      assert.match(messages, /resolverPathOfficialRanges/u);
      assert.match(messages, /resolverPathCurrentCandidate/u);
      assert.match(messages, /resolverPathPreferredIp/u);
      assert.match(messages, /candidateResolutionUnavailableTitle/u);
    }
  });

  it("gates scans and candidate publishing on the applied managed state", () => {
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareOptimization.ts",
    );
    assert.match(controller, /optimization\.value\?\.enabled === true/u);
    assert.match(controller, /if \(!optimizationApplied\.value\)/u);
    assert.match(controller, /optimization\.value\?\.scanReady === true/u);
    assert.match(controller, /if \(!optimizationScanReady\.value\)/u);

    const optimization = readOptimizationUi();
    assert.match(optimization, /!optimizationApplied/u);
    assert.match(optimization, /reconcileRequiredDescription/u);
  });

  it("validates a user-specified preferred IP before recommending it", () => {
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareOptimization.ts",
    );
    const card = readOptimizationUi();
    const backend = [
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/api.rs",
      ),
    ].join("\n");

    assert.match(controller, /preferredCandidateIp/u);
    assert.match(controller, /startOptimizationScan\(/u);
    assert.match(card, /preferredIpValidated === true/u);
    assert.match(card, /preferredIpValidated === false/u);
    assert.match(
      card,
      /aria-describedby="optimization-preferred-ip-description"/u,
    );
    assert.match(backend, /normalize_preferred_ip/u);
    assert.match(backend, /bundled_cloudflare_prefixes/u);
    assert.match(backend, /retain_shortlist_with_priority/u);
    assert.match(backend, /candidate\.business_validated/u);
  });

  it("distinguishes Cloudflare for SaaS setup from validation readiness", () => {
    const optimization = [
      readOptimizationUi(),
      readSource(
        "../src/views/tunnel/cloudflare/cloudflareOptimizationPresentation.ts",
      ),
    ].join("\n");
    assert.match(optimization, /no active business or capability hostname/u);
    assert.match(optimization, /cloudflareSaasRequiredTitle/u);
    assert.match(optimization, /cloudflareSaasRequiredDescription/u);
    assert.match(optimization, /optimizationScan\.value\?\.errorCode/u);
    assert.match(optimization, /probe\?\.reasonCode/u);
    assert.match(optimization, /cloudflare-saas-validation-pending/u);
    assert.match(optimization, /cloudflare-resource-conflict/u);
    assert.match(optimization, /cloudflare-optimization-not-ready/u);
    assert.match(optimization, /!optimizationScanReady/u);
    assert.match(optimization, /probe\.status === "pending"/u);
    assert.ok(
      optimization.match(/presentation\.capabilityProbeMessage/gu)?.length ===
        2,
      "the capability alert and technical status should use the same localized message",
    );

    const schemas = readContract().components.schemas;
    assert.ok(schemas.CloudflareOptimizationScanData.properties?.errorCode);
    assert.ok(
      schemas.CloudflareOptimizationCapabilityProbeData.properties?.reasonCode,
    );
    assert.ok(schemas.CloudflareOptimizationStateData.properties?.scanReady);
    assert.ok(
      schemas.CloudflareOptimizationStateData.properties
        ?.scanReadinessErrorCode,
    );
    assert.ok(
      schemas.CloudflareOptimizationDomainData.properties?.hostnameStatus,
    );
    assert.ok(
      schemas.CloudflareOptimizationCapabilityProbeData.properties?.status?.enum?.includes(
        "probe-failed",
      ),
    );

    const backend = [
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/api.rs",
      ),
      readSource(
        "../../server-admin-rs/src/tunnels/cloudflared/optimization/scheduler.rs",
      ),
    ].join("\n");
    assert.match(backend, /CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE/u);
    assert.match(backend, /CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE/u);
    assert.match(backend, /CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE/u);
    assert.match(backend, /OPTIMIZATION_NOT_READY_ERROR_CODE/u);
    assert.match(backend, /"errorCode": error_code/u);
    assert.match(backend, /"reasonCode"\.to_string\(\)/u);
    assert.match(backend, /"scanReady": scan_ready/u);
    assert.match(
      backend,
      /"scanReadinessErrorCode": scan_readiness_error_code/u,
    );
    assert.match(
      backend,
      /recoverable_fn_knock_custom_hostname_from_snapshot/u,
    );
    assert.match(backend, /"recover"/u);
    assert.match(
      backend,
      /scan_due && scan_validation_hostname\(&ownership\)\.is_none\(\)/u,
    );
    assert.match(backend, /custom_hostname_can_validate_candidates/u);
    assert.match(backend, /refresh_tracked_custom_hostname_statuses/u);
    assert.match(backend, /capability_probe_failure_state/u);
    assert.match(backend, /"hostnameStatus"/u);
    assert.match(backend, /preferredEdgeProbeFailed/u);

    const messages = readSource(
      "../../../packages/i18n/src/messages/admin/zh-CN.ts",
    );
    assert.match(messages, /账号与域名相关资源授予完整编辑权限/u);
    assert.match(
      messages,
      /如果已在上方配置 Cloudflare API Token，此处无需填写/u,
    );
    assert.match(messages, /SSL\/TLS → 自定义主机名/u);
    assert.match(messages, /100 个自定义主机名/u);
    assert.match(messages, /绑定付款方式不会立即扣费/u);
    assert.match(messages, /Cloudflare for SaaS 已启用/u);
    assert.match(messages, /主机名和证书状态均变为“有效”/u);
    assert.match(messages, /无需重复开通功能或绑定付款方式/u);
    assert.match(messages, /这不是证书签发等待/u);
    assert.match(messages, /保留现有有效证书并无损恢复/u);
    assert.match(messages, /优选验证尚未就绪/u);
    assert.match(messages, /优选入口验证失败/u);

    const maintenance = readSource(
      "../../server-admin-rs/src/system/maintenance/routes.rs",
    );
    assert.match(maintenance, /cloudflared::cleanup_before_data_clear/u);
  });
});
