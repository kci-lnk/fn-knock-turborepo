import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  buildDnsCredentialTransferSuggestion as build,
  resolveDnsCredentialBridges,
} from "../src/lib/dns-credential-bridge";

const cases: [
  string,
  string,
  Record<string, string>,
  Record<string, string>,
][] = [
  [
    "cloudflare",
    "dns_cf",
    { api_token: "token", zone_id: "zone" },
    { CF_Token: "token", CF_Zone_ID: "zone" },
  ],
  [
    "alidns",
    "dns_ali",
    { access_key_id: "id", access_key_secret: "secret" },
    { Ali_Key: "id", Ali_Secret: "secret" },
  ],
  [
    "esa",
    "dns_ali",
    { access_key_id: "id", access_key_secret: "secret" },
    { Ali_Key: "id", Ali_Secret: "secret" },
  ],
  [
    "dnspod",
    "dns_dp",
    { token_id: "id", token_key: "secret" },
    { DP_Id: "id", DP_Key: "secret" },
  ],
  ...["tencentcloud", "edgeone", "edgeone_cname"].map(
    (provider): (typeof cases)[number] => [
      provider,
      "dns_tencent",
      { secret_id: "id", secret_key: "secret" },
      { Tencent_SecretId: "id", Tencent_SecretKey: "secret" },
    ],
  ),
  [
    "dnshe",
    "dns_dnshe",
    { api_key: "id", api_secret: "secret" },
    { DNSHE_API_KEY: "id", DNSHE_API_SECRET: "secret" },
  ],
  [
    "godaddy",
    "dns_gd",
    { api_key: "id", api_secret: "secret" },
    { GD_Key: "id", GD_Secret: "secret" },
  ],
  [
    "porkbun",
    "dns_porkbun",
    { api_key: "id", secret_api_key: "secret" },
    { PORKBUN_API_KEY: "id", PORKBUN_SECRET_API_KEY: "secret" },
  ],
  ["dynv6", "dns_dynv6", { token: "token" }, { DYNV6_TOKEN: "token" }],
  ["duckdns", "dns_duckdns", { token: "token" }, { DuckDNS_Token: "token" }],
  [
    "baiducloud",
    "dns_baiducloud",
    { access_key_id: "id", secret_access_key: "secret" },
    { BAIDU_ACCESS_KEY_ID: "id", BAIDU_SECRET_ACCESS_KEY: "secret" },
  ],
];

describe("DNS credential compatibility", () => {
  for (const [ddns, acme, ddnsCredentials, acmeCredentials] of cases) {
    for (const target of ["acme", "ddns"] as const) {
      it(`${ddns} → ${target}: transfers only mapped fields and preserves existing values`, () => {
        const expected = target === "acme" ? acmeCredentials : ddnsCredentials;
        const options = {
          target,
          providerId: target === "acme" ? acme : ddns,
          sourceProviderId: target === "acme" ? ddns : acme,
          sourceCredentials: {
            ...(target === "acme" ? ddnsCredentials : acmeCredentials),
            domain: "do-not-copy.test",
            ttl: "300",
          },
          targetCredentials: {},
        };
        assert.deepEqual(build(options)?.patch, expected);
        const [firstKey, firstValue] = Object.entries(expected)[0]!;
        const partial = build({
          ...options,
          targetCredentials: { [firstKey]: ` ${firstValue} ` },
        });
        const remaining = { ...expected };
        delete remaining[firstKey];
        assert.deepEqual(partial?.patch ?? {}, remaining);
        assert.equal(build({ ...options, targetCredentials: expected }), null);
      });
    }
  }
  it("resolves every Tencent and Alibaba source", () => {
    assert.deepEqual(
      resolveDnsCredentialBridges("acme", "dns_tencent").map(
        (b) => b.ddnsProvider,
      ),
      ["tencentcloud", "edgeone", "edgeone_cname"],
    );
    assert.deepEqual(
      resolveDnsCredentialBridges("acme", "dns_ali").map((b) => b.ddnsProvider),
      ["alidns", "esa"],
    );
    assert.deepEqual(resolveDnsCredentialBridges("ddns", "huaweicloud"), []);
    assert.deepEqual(resolveDnsCredentialBridges("ddns", "dynu"), []);
  });
  it("blocks mismatched pairs and cannot prove a pair when the source ID is missing", () => {
    for (const sourceCredentials of [
      { Tencent_SecretId: "other", Tencent_SecretKey: "secret" },
      { Tencent_SecretKey: "secret" },
    ]) {
      const result = build({
        target: "ddns",
        providerId: "tencentcloud",
        sourceProviderId: "dns_tencent",
        sourceCredentials,
        targetCredentials: { secret_id: "mine" },
      });
      assert.deepEqual(result?.patch, {});
      assert.deepEqual(result?.conflictingFields, ["secret_id"]);
    }
  });
  it("does not confuse DNSPod tokens, zone-only configurations, or alternate auth schemes", () => {
    assert.equal(
      build({
        target: "ddns",
        providerId: "tencentcloud",
        sourceProviderId: "dns_dp",
        sourceCredentials: { DP_Id: "id", DP_Key: "key" },
        targetCredentials: {},
      }),
      null,
    );
    assert.equal(
      build({
        target: "ddns",
        providerId: "cloudflare",
        sourceProviderId: "dns_cf",
        sourceCredentials: {
          CF_Key: "key",
          CF_Email: "email",
          CF_Zone_ID: "zone",
        },
        targetCredentials: {},
      }),
      null,
    );
    for (const [
      providerId,
      sourceProviderId,
      sourceCredentials,
      targetCredentials,
    ] of [
      [
        "dns_cf",
        "cloudflare",
        { api_token: "token" },
        { CF_Key: "key", CF_Email: "email" },
      ],
      ["dns_dynv6", "dynv6", { token: "token" }, { KEY: "ssh" }],
    ] as [string, string, Record<string, string>, Record<string, string>][]) {
      assert.deepEqual(
        build({
          target: "acme",
          providerId,
          sourceProviderId,
          sourceCredentials,
          targetCredentials,
        })?.patch,
        {},
      );
    }
  });
  it("ignores empty credentials and trims reusable values", () => {
    const options = {
      target: "acme" as const,
      providerId: "dns_duckdns",
      sourceProviderId: "duckdns",
      targetCredentials: {},
    };
    assert.equal(
      build({ ...options, sourceCredentials: { token: "  " } }),
      null,
    );
    assert.deepEqual(
      build({ ...options, sourceCredentials: { token: "  token  " } })?.patch,
      { DuckDNS_Token: "token" },
    );
  });
});
