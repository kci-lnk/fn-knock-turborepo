import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("managed Cloudflare Tunnel", () => {
  it("exposes the credential, reconcile, scan, apply, and fallback contracts", () => {
    const source = readSource("../src/lib/api/tunnel.ts");
    for (const route of [
      "/cloudflared/cloudflare/credential",
      "/cloudflared/cloudflare/state",
      "/cloudflared/reconcile/preview",
      "/cloudflared/reconcile/apply",
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

  it("keeps Tunnel and API tokens write-only in the frontend contract", () => {
    const apiSource = readSource("../src/lib/api/tunnel.ts");
    const configContract = apiSource.slice(
      apiSource.indexOf("export type CloudflaredConfig"),
      apiSource.indexOf("export type CloudflareTunnelSummary"),
    );
    assert.match(configContract, /apiTokenConfigured: boolean/u);
    assert.match(configContract, /tunnelTokenConfigured: boolean/u);
    assert.doesNotMatch(configContract, /\btoken:\s*string/u);

    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    assert.match(controller, /token\.value = ""/u);
    assert.doesNotMatch(controller, /token\.value\s*=\s*config\.token/u);
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

    const managed = readSource(
      "../src/views/tunnel/cloudflare/CloudflareManagedTunnelCard.vue",
    );
    assert.match(managed, /ConfigCollapsibleCard/u);
    assert.match(managed, /reconcilePlan\.operations/u);
    assert.match(managed, /toggleTakeover/u);
    assert.match(managed, /planWarningLabel/u);
    assert.match(managed, /warningCodes/u);
    assert.match(managed, /conflictMessageLabel/u);
    assert.match(managed, /conflict\.details\.records/u);
    assert.match(managed, /dnsOwnerLabel/u);
    assert.match(managed, /reconcileAttentionToken/u);
    assert.match(managed, /operationTargetLabel/u);
    assert.match(managed, /keepDeleted/u);
    assert.match(managed, /toLocaleString\(locale\.value\)/u);
    assert.doesNotMatch(managed, /text-muted-foreground">Tunnel</u);

    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
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
    assert.match(optimization, /toLocaleString\(locale\.value\)/u);
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
    const backend = readSource(
      "../../server-admin-rs/src/tunnels/cloudflared/optimization.rs",
    );
    assert.match(backend, /cloudflare-dns\.com\/dns-query/u);
    assert.match(backend, /dns\.google\/resolve/u);
    assert.match(backend, /candidate_ip_is_cloudflare/u);
    assert.match(backend, /source_hostnames/u);
    assert.match(backend, /business_validated/u);
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
    }
  });

  it("gates scans and candidate publishing on the applied managed state", () => {
    const controller = readSource(
      "../src/views/tunnel/cloudflare/useCloudflareTunnelController.ts",
    );
    assert.match(controller, /optimization\.value\?\.enabled === true/u);
    assert.match(controller, /if \(!optimizationApplied\.value\)/u);
    assert.match(controller, /optimization\.value\?\.scanReady === true/u);
    assert.match(controller, /if \(!optimizationScanReady\.value\)/u);

    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
    assert.match(optimization, /!optimizationApplied/u);
    assert.match(optimization, /reconcileRequiredDescription/u);
  });

  it("distinguishes Cloudflare for SaaS setup from validation readiness", () => {
    const optimization = readSource(
      "../src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    );
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
      optimization.match(/\{\{ capabilityProbeMessage \}\}/gu)?.length === 2,
      "the capability alert and technical status should use the same localized message",
    );

    const api = readSource("../src/lib/api/tunnel.ts");
    assert.match(api, /errorCode\?: string \| null/u);
    assert.match(api, /reasonCode\?: string/u);
    assert.match(api, /scanReady: boolean/u);
    assert.match(api, /scanReadinessErrorCode: string \| null/u);

    const backend = readSource(
      "../../server-admin-rs/src/tunnels/cloudflared/optimization.rs",
    );
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

    const maintenance = readSource(
      "../../server-admin-rs/src/system/maintenance/routes.rs",
    );
    assert.match(maintenance, /cloudflared::cleanup_before_data_clear/u);
  });
});
