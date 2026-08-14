import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  cleanHostLocationPath,
  cloneLocation,
  createDefaultLocation,
  DEFAULT_RESPONSE_CONTENT_TYPE,
} from "../src/views/system-settings/gateway-locations/gatewayLocationModel";
import {
  applyStreamMappingSubmission,
  compareStreamMappings,
  formatMappingLabel,
  getMappingKey,
  isObviousLocalStreamTargetLoop,
  normalizeProtocolSelection,
  normalizeStreamMapping,
  removeStreamMapping,
  updateStreamMappingComment,
} from "../src/views/stream-mappings/streamMappingModel";
import {
  durationUnits,
  ipGrantDurationUnits,
  splitDuration,
  toDurationSeconds,
} from "../src/views/system-settings/session-settings/sessionDurationModel";
import {
  cloneSmartConnectDetails,
  getComparableSmartConnectConfig,
  hasUnsavedSmartConnectDraft,
  resolveSelectedIpv4,
} from "../src/views/system-settings/smart-connect/smartConnectModel";
import {
  buildAcmeCredentialsPayload,
  getProviderCredentialFields,
  getProviderGroupKey,
  getSatisfiedCredentialScheme,
  normalizeProviderCredentials,
} from "../src/views/ssl-settings/acme-application/acmeApplicationModel";
import {
  replaceFrpcOverviewItem,
  summarizeFrpcContent,
} from "../src/views/tunnel/frp/frpcInstanceModel";
import { analyzeCloudflaredLogs } from "../src/views/tunnel/cloudflare/cloudflaredLogAnalysis";
import {
  supervisorAllowsStart,
  supervisorAllowsStop,
  supervisorTone,
} from "../src/lib/tunnelSupervisorModel";
import {
  buildIpLocationSettingsPayload,
  isHttpUrl,
  normalizeIpLocationBaseUrl,
  normalizeIpLocationSettings,
  OFFICIAL_CIDR_URL,
  OFFICIAL_IP_LOOKUP_URL,
} from "../src/views/system-settings/ip-location/ipLocationSettingsModel";
import {
  buildSessionMobilityTimeline,
  formatSessionMobilityDuration,
  getSessionMobilityTimelineSpan,
  middleEllipsis,
} from "../src/views/session-management/mobility/sessionMobilityModel";
import {
  hasConfiguredProviderFields,
  isProviderConfigEqual,
  normalizeProviderConfigForComparison,
  type Provider,
} from "../src/views/ddns-management/model";
import {
  alignTimeSeriesData,
  buildTimeSeriesLegendItems,
  hasRenderableTimeSeriesData,
  toTimeSeriesTimestampMs,
} from "../src/components/charts/timeSeriesChartModel";
import type { SessionMobilityEvent } from "../src/types";
import type {
  AcmeDnsProvider,
  FrpcInstanceStatus,
  FrpcInstancesOverview,
} from "../src/lib/api";
import type {
  HostLocation,
  SmartConnectDetails,
  StreamMapping,
} from "../src/types";

describe("gateway location model", () => {
  it("normalizes dot segments without accepting a missing leading slash", () => {
    assert.equal(cleanHostLocationPath(" /api/../v1//./users "), "/v1/users");
    assert.equal(cleanHostLocationPath("api/v1"), "api/v1");
    assert.equal(cleanHostLocationPath("/"), "/");
  });

  it("clones nested response data and restores safe response defaults", () => {
    const source = {
      ...createDefaultLocation(),
      path: "/health",
      action: "response",
      response: {
        status: 204,
        content_type: "",
        headers: { "X-Test": "yes" },
        body: "ok",
      },
    } satisfies HostLocation;
    const cloned = cloneLocation(source);

    assert.notEqual(cloned.response, source.response);
    assert.notEqual(cloned.response.headers, source.response.headers);
    assert.equal(cloned.response.content_type, DEFAULT_RESPONSE_CONTENT_TYPE);
    cloned.response.headers["X-Test"] = "changed";
    assert.equal(source.response.headers["X-Test"], "yes");
  });
});

describe("stream mapping model", () => {
  it("detects obvious same-port local forwarding loops", () => {
    for (const target of [
      "localhost:5555",
      "127.0.0.1:5555",
      "127.0.0.42:5555",
      "[::1]:5555",
      "[::ffff:127.0.0.1]:5555",
    ]) {
      assert.equal(isObviousLocalStreamTargetLoop(target, 5555), true);
    }
    assert.equal(
      isObviousLocalStreamTargetLoop("127.0.0.1:5555", 15555),
      false,
    );
    assert.equal(
      isObviousLocalStreamTargetLoop("192.0.2.20:5555", 5555),
      false,
    );
  });

  it("normalizes protocol selections in stable TCP/UDP order", () => {
    assert.deepEqual(normalizeProtocolSelection(["udp", "tcp", "udp"]), [
      "tcp",
      "udp",
    ]);
    assert.deepEqual(normalizeProtocolSelection([]), ["tcp"]);
  });

  it("builds stable keys, labels, and port-first ordering", () => {
    const mappings: StreamMapping[] = [
      { protocol: "udp", listen_port: 443, target: "a:443", use_auth: true },
      { protocol: "tcp", listen_port: 80, target: "a:80", use_auth: false },
      { protocol: "tcp", listen_port: 443, target: "a:443", use_auth: true },
    ];
    mappings.sort(compareStreamMappings);

    assert.deepEqual(mappings.map(getMappingKey), [
      "tcp:80",
      "tcp:443",
      "udp:443",
    ]);
    assert.equal(formatMappingLabel(mappings[2]!), "UDP/443");
    const normalized = normalizeStreamMapping({
      ...mappings[0]!,
      protocol: undefined as unknown as "tcp",
      comment: "  Web service  ",
    });
    assert.equal(normalized.protocol, "tcp");
    assert.equal(normalized.comment, "Web service");
    assert.equal(normalizeStreamMapping(mappings[1]!).comment, "");
  });

  it("rebases queued edits on the latest mapping collection", () => {
    const mappings: StreamMapping[] = [
      {
        protocol: "tcp",
        listen_port: 22,
        target: "a:22",
        use_auth: true,
        comment: "",
      },
      {
        protocol: "tcp",
        listen_port: 80,
        target: "a:80",
        use_auth: true,
        comment: "",
      },
    ];
    const first = updateStreamMappingComment(mappings, "tcp:22", "SSH");
    const second = updateStreamMappingComment(first, "tcp:80", "Web");
    assert.deepEqual(
      second.map((mapping) => mapping.comment),
      ["SSH", "Web"],
    );
    assert.deepEqual(
      removeStreamMapping(second, "tcp:22").map(getMappingKey),
      ["tcp:80"],
    );
    assert.equal(
      applyStreamMappingSubmission(second, {
        editingKey: "tcp:80",
        mappings: [
          {
            protocol: "tcp",
            listen_port: 8080,
            target: "a:8080",
            use_auth: false,
            comment: "Web",
          },
        ],
      }).map(getMappingKey)[1],
      "tcp:8080",
    );
  });

  it("removes a legacy local-loop mapping without retaining it in the payload", () => {
    const legacy: StreamMapping = {
      protocol: "udp",
      listen_port: 12333,
      target: "127.0.0.1:12333",
      use_auth: true,
      comment: "legacy",
    };

    assert.equal(
      isObviousLocalStreamTargetLoop(legacy.target, legacy.listen_port),
      true,
    );
    assert.deepEqual(removeStreamMapping([legacy], getMappingKey(legacy)), []);
  });
});

describe("session duration model", () => {
  it("round-trips duration fields and prefers the largest exact unit", () => {
    assert.equal(toDurationSeconds({ value: 2, unit: "hour" }), 7200);
    assert.equal(toDurationSeconds({ value: 2, unit: "month" }), 5_184_000);
    assert.deepEqual(splitDuration(7200), { value: 2, unit: "hour" });
    assert.deepEqual(splitDuration(60 * 24 * 3600), {
      value: 2,
      unit: "month",
    });
    assert.deepEqual(splitDuration(90), { value: 90, unit: "second" });
  });

  it("honors constrained unit sets", () => {
    assert.deepEqual(splitDuration(3600, ipGrantDurationUnits), {
      value: 1,
      unit: "hour",
    });
    assert.deepEqual(
      durationUnits.map(({ value, seconds }) => ({ value, seconds })),
      [
        { value: "second", seconds: 1 },
        { value: "minute", seconds: 60 },
        { value: "hour", seconds: 3600 },
        { value: "day", seconds: 24 * 3600 },
        { value: "week", seconds: 7 * 24 * 3600 },
        { value: "month", seconds: 30 * 24 * 3600 },
        { value: "year", seconds: 365 * 24 * 3600 },
      ],
    );
  });
});

describe("smart connect model", () => {
  const details: SmartConnectDetails = {
    config: { enabled: false, selected_ipv4: "192.168.1.2" },
    availability: { available: true, reason: "" },
    dnsmasq: {
      installed: true,
      initialized: true,
      service_active: true,
      install_state: { status: "installed", progress: 100, message: "ok" },
      runtime: { managed_rule_count: 1, synced_domains: ["example.com"] },
    },
    domains: ["example.com"],
    local_ip_options: [
      { value: "192.168.1.2", label: "LAN", interface: "eth0" },
    ],
  };

  it("deep-clones mutable nested collections", () => {
    const cloned = cloneSmartConnectDetails(details);
    cloned.domains.push("new.example.com");
    cloned.dnsmasq.runtime.synced_domains.push("new.example.com");
    cloned.local_ip_options[0]!.label = "Changed";

    assert.deepEqual(details.domains, ["example.com"]);
    assert.deepEqual(details.dnsmasq.runtime.synced_domains, ["example.com"]);
    assert.equal(details.local_ip_options[0]!.label, "LAN");
  });

  it("does not mark a disabled form dirty solely for its hidden IP draft", () => {
    assert.deepEqual(
      getComparableSmartConnectConfig(
        { enabled: false, selected_ipv4: "" },
        "192.168.1.2",
      ),
      { enabled: false, selected_ipv4: "192.168.1.2" },
    );
    assert.equal(
      hasUnsavedSmartConnectDraft(details, {
        enabled: false,
        selected_ipv4: "",
      }),
      false,
    );
    assert.equal(
      resolveSelectedIpv4("", details.local_ip_options),
      "192.168.1.2",
    );
  });
});

describe("ACME application model", () => {
  const provider: AcmeDnsProvider = {
    dnsType: "dns_example",
    label: "Example DNS",
    group: "\u5e38\u7528",
    credentialSchemes: [
      {
        id: "token",
        label: "API Token",
        fields: [
          { key: "TOKEN", required: true },
          { key: "ACCOUNT", required: false },
        ],
      },
      {
        id: "key-secret",
        label: "Key and Secret",
        fields: [
          { key: "KEY", required: true },
          { key: "SECRET", required: true },
          { key: "ACCOUNT", required: false },
        ],
      },
    ],
  };

  it("deduplicates fields while preserving credential scheme order", () => {
    assert.deepEqual(
      getProviderCredentialFields(provider).map((field) => field.key),
      ["TOKEN", "ACCOUNT", "KEY", "SECRET"],
    );
    assert.deepEqual(
      normalizeProviderCredentials(provider, {
        TOKEN: "saved-token",
        SECRET: "saved-secret",
        UNUSED: "drop-me",
      }),
      {
        TOKEN: "saved-token",
        ACCOUNT: "",
        KEY: "",
        SECRET: "saved-secret",
      },
    );
  });

  it("accepts any complete scheme and ignores optional fields", () => {
    assert.equal(
      getSatisfiedCredentialScheme(provider, { TOKEN: " token " })?.id,
      "token",
    );
    assert.equal(
      getSatisfiedCredentialScheme(provider, {
        KEY: "key",
        SECRET: "secret",
      })?.id,
      "key-secret",
    );
    assert.equal(getSatisfiedCredentialScheme(provider, { KEY: "key" }), null);
  });

  it("trims submit credentials and preserves provider group semantics", () => {
    assert.deepEqual(
      buildAcmeCredentialsPayload({
        " TOKEN ": " value ",
        EMPTY: "   ",
        " ": "ignored",
      }),
      { TOKEN: "value" },
    );
    assert.equal(getProviderGroupKey("\u5e38\u7528"), "common");
    assert.equal(getProviderGroupKey("\u81ea\u5efa/\u9ad8\u7ea7"), "customAdvanced");
    assert.equal(getProviderGroupKey("Partner"), "Partner");
    assert.equal(getProviderGroupKey(undefined), "other");
  });
});

describe("FRP instance model", () => {
  const createInstance = (
    id: string,
    running: boolean,
  ): FrpcInstanceStatus => ({
    id,
    name: id,
    isPrimary: id === "primary",
    configPath: `/tmp/${id}.toml`,
    workDir: "/tmp",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    sortOrder: 0,
    desiredRunning: running,
    running,
    attached: running,
    pid: running ? 123 : null,
    startedAt: null,
    stoppedAt: null,
    lastExitCode: null,
    lastMessage: null,
    supervisor: {
      state: running ? "running" : "stopped",
      desiredRunning: running,
      running,
      attached: running,
      pid: running ? 123 : null,
      restartCount: 0,
      consecutiveFailures: 0,
      nextRestartAt: null,
      startedAt: null,
      stoppedAt: null,
      lastFailure: null,
      lastMessage: null,
    },
    summary: {
      serverAddr: "frp.example.com",
      serverPort: "7000",
      localPort: "7999",
      remotePort: "8080",
    },
  });

  it("derives a summary from TOML and falls back safely for invalid content", () => {
    const summary = summarizeFrpcContent(
      [
        'serverAddr = "frp.example.com"',
        "serverPort = 7001",
        "[[proxies]]",
        'name = "reproxy"',
        'type = "tcp"',
        "localPort = 7999",
        "remotePort = 8080",
      ].join("\n"),
      "7999",
    );
    assert.deepEqual(summary, {
      serverAddr: "frp.example.com",
      serverPort: "7001",
      localPort: "7999",
      remotePort: "8080",
    });
    assert.deepEqual(summarizeFrpcContent("[[[", "9000"), {
      serverAddr: "",
      serverPort: "7000",
      localPort: "9000",
      remotePort: "",
    });
  });

  it("replaces one polled instance and recalculates the running count", () => {
    const overview: FrpcInstancesOverview = {
      initialized: true,
      platform: "linux-amd64",
      primaryInstanceId: "primary",
      total: 2,
      extraCount: 1,
      runningCount: 1,
      defaults: { local_port: "7999" },
      items: [createInstance("primary", true), createInstance("extra", false)],
    };
    const updated = replaceFrpcOverviewItem(
      overview,
      createInstance("extra", true),
    );

    assert.equal(updated.runningCount, 2);
    assert.equal(updated.items[1]?.running, true);
    assert.equal(overview.items[1]?.running, false);
  });
});

describe("Cloudflared log analysis", () => {
  it("finds the newest TLS hostname mismatch and resolves its origin host", () => {
    const older =
      "tls: failed to verify certificate: x509: certificate is valid for old.local, not old.example.com dest=https://old.local:443";
    const newest =
      "ERR tls: failed to verify certificate: x509: certificate is valid for localhost, 127.0.0.1, not auth.example.com dest=https://127.0.0.1:7999";
    const result = analyzeCloudflaredLogs([older, "connected", newest]);

    assert.deepEqual(result, {
      reason: "origin_tls_hostname_mismatch",
      requestedHost: "auth.example.com",
      certificateHosts: ["localhost", "127.0.0.1"],
      originUrl: "https://127.0.0.1:7999",
      originHost: "127.0.0.1",
      evidence: newest,
    });
  });

  it("ignores unrelated and incomplete log entries", () => {
    assert.equal(analyzeCloudflaredLogs(["connection registered"]), null);
    assert.equal(
      analyzeCloudflaredLogs([
        "x509: certificate is valid for localhost, not auth.example.com",
      ]),
      null,
    );
  });
});

describe("tunnel supervisor model", () => {
  it("keeps backoff controllable without presenting it as stopped", () => {
    const backoff = {
      state: "backoff",
      desiredRunning: true,
      running: false,
    } as const;
    assert.equal(supervisorTone(backoff), "warning");
    assert.equal(supervisorAllowsStart(backoff), false);
    assert.equal(supervisorAllowsStop(backoff), true);
  });

  it("allows start only after guarding has been explicitly stopped", () => {
    const stopped = {
      state: "stopped",
      desiredRunning: false,
      running: false,
    } as const;
    assert.equal(supervisorTone(stopped), "muted");
    assert.equal(supervisorAllowsStart(stopped), true);
    assert.equal(supervisorAllowsStop(stopped), false);
  });

  it("keeps a process controllable when termination failed after intent was cleared", () => {
    const terminationFailed = {
      state: "running",
      desiredRunning: false,
      running: true,
    } as const;
    assert.equal(supervisorAllowsStart(terminationFailed), false);
    assert.equal(supervisorAllowsStop(terminationFailed), true);
  });
});

describe("IP location settings model", () => {
  it("normalizes trailing slashes and uses official endpoints in online mode", () => {
    assert.equal(
      normalizeIpLocationBaseUrl(" https://example.com/api/// "),
      "https://example.com/api",
    );
    assert.deepEqual(
      buildIpLocationSettingsPayload({
        ipLookupMode: "online",
        ipLookupUrl: "http://ignored.local",
        cidrMode: "online",
        cidrUrl: "http://ignored.local",
      }),
      {
        ip_lookup_mode: "online",
        ip_lookup_url: OFFICIAL_IP_LOOKUP_URL,
        cidr_mode: "online",
        cidr_url: OFFICIAL_CIDR_URL,
      },
    );
  });

  it("preserves normalized custom endpoints and only accepts HTTP URLs", () => {
    assert.deepEqual(
      normalizeIpLocationSettings({
        ip_lookup_mode: "custom",
        ip_lookup_url: "http://127.0.0.1:30661/",
        cidr_mode: "custom",
        cidr_url: "https://cidr.example.com///",
      }),
      {
        ip_lookup_mode: "custom",
        ip_lookup_url: "http://127.0.0.1:30661",
        cidr_mode: "custom",
        cidr_url: "https://cidr.example.com",
      },
    );
    assert.equal(isHttpUrl("https://example.com"), true);
    assert.equal(isHttpUrl("ftp://example.com"), false);
    assert.equal(isHttpUrl("not-a-url"), false);
  });
});

describe("session mobility model", () => {
  const translate = (
    key: string,
    params?: Record<string, string | number>,
  ) => `${key}${params ? `:${JSON.stringify(params)}` : ""}`;

  it("builds a chronological timeline without mutating API event order", () => {
    const events: SessionMobilityEvent[] = [
      {
        version: 1,
        kind: "drift",
        happenedAt: "2026-01-01T02:30:00Z",
        source: "fnos-token",
        fromIp: "10.0.0.2",
        toIp: "10.0.0.3",
      },
      {
        version: 1,
        kind: "login",
        happenedAt: "2026-01-01T00:00:00Z",
        source: "login",
        toIp: "10.0.0.1",
      },
    ];
    const timeline = buildSessionMobilityTimeline(events, translate);

    assert.equal(timeline[0]?.event.kind, "login");
    assert.equal(timeline[1]?.event.kind, "drift");
    assert.match(timeline[1]?.gapLabel || "", /hoursMinutes/);
    assert.equal(getSessionMobilityTimelineSpan(timeline), 9_000_000);
    assert.equal(events[0]?.kind, "drift");
  });

  it("formats duration boundaries and stable middle ellipses", () => {
    assert.match(formatSessionMobilityDuration(59_999, translate), /lessThanMinute/);
    assert.match(formatSessionMobilityDuration(60_000, translate), /minutes/);
    assert.match(formatSessionMobilityDuration(3_660_000, translate), /hoursMinutes/);
    assert.match(formatSessionMobilityDuration(90_000_000, translate), /daysHours/);
    assert.equal(middleEllipsis("abcdefghijklmnop", 9), "abcd……mnop");
    assert.equal(middleEllipsis("short", 9), "short");
  });
});

describe("DDNS primary provider config model", () => {
  const provider: Provider = {
    name: "cloudflare",
    label: "Cloudflare",
    fields: [
      { key: "token", label: "Token", type: "password" },
      { key: "zone", label: "Zone", type: "text" },
    ],
  };

  it("detects only declared non-empty provider credentials", () => {
    assert.equal(hasConfiguredProviderFields(provider, { token: "  " }), false);
    assert.equal(
      hasConfiguredProviderFields(provider, {
        token: "secret",
        unrelated: "value",
      }),
      true,
    );
    assert.equal(hasConfiguredProviderFields(null, { token: "secret" }), false);
  });

  it("compares normalized common config keys in stable order", () => {
    assert.equal(isProviderConfigEqual({}, { ip_source: "public" }), true);
    assert.deepEqual(
      Object.keys(
        normalizeProviderConfigForComparison({ zone: "example.com", token: "x" }),
      ),
      [...Object.keys(normalizeProviderConfigForComparison({})), "token", "zone"].sort(),
    );
    assert.equal(
      isProviderConfigEqual({ token: "x" }, { token: "y" }),
      false,
    );
  });
});

describe("time series chart model", () => {
  it("normalizes seconds and aligns sparse series chronologically", () => {
    assert.equal(toTimeSeriesTimestampMs(1_700_000_000), 1_700_000_000_000);
    const data = alignTimeSeriesData([
      {
        name: "in",
        color: "#0f0",
        data: [
          [2, 20],
          [1, 10],
          ["invalid", 99],
        ],
      },
      {
        name: "out",
        color: "#00f",
        data: [[2, 5]],
      },
    ]);
    assert.deepEqual(data, [
      [1000, 2000],
      [10, 20],
      [null, 5],
    ]);
    assert.equal(hasRenderableTimeSeriesData(data), true);
    assert.equal(
      hasRenderableTimeSeriesData([[1000], [null]]),
      false,
    );
  });

  it("keeps legend labels stable for unnamed series", () => {
    assert.deepEqual(
      buildTimeSeriesLegendItems([
        { name: " ", color: "red", data: [] },
        { name: "Traffic", color: "blue", data: [] },
      ]),
      [
        { name: "Series 1", color: "red" },
        { name: "Traffic", color: "blue" },
      ],
    );
  });
});
