/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ref } from "vue";

import {
  isValidDDNSDomainTarget,
  isSameOrSubdomain,
  normalizeDDNSDomainTargetInput,
  parseDDNSDomainTargets,
  validateDDNSDomainTargets,
  type DDNSDomainTargetsCapability,
} from "../src/lib/ddns-domain";
import {
  validateDDNSCommonConfig,
  validateDDNSTargetConfig,
  type Provider,
} from "../src/views/ddns-management/model";
import { useDDNSDomainField } from "../src/views/ddns-management/useDDNSDomainField";

const pairCapability = (
  rootField?: "root_domain" | "site_name",
): DDNSDomainTargetsCapability => ({
  mode: "single_or_wildcard_root_pair",
  ...(rootField ? { rootField } : {}),
});

const provider = (
  name: string,
  capability?: DDNSDomainTargetsCapability,
): Provider => ({
  name,
  label: name,
  fields: [
    {
      key: "root_domain",
      label: "Root Domain",
      type: "text",
      required: true,
    },
    {
      key: "domain",
      label: "Full Domain",
      type: "text",
      required: true,
      description: "Domain description.",
    },
  ],
  capabilities: capability ? { domainTargets: capability } : undefined,
});

describe("DDNS domain target parser", () => {
  it("matches zone roots only at DNS label boundaries", () => {
    assert.equal(isSameOrSubdomain("wxlnk.com", "wxlnk.com"), true);
    assert.equal(isSameOrSubdomain("r.wxlnk.com", "wxlnk.com"), true);
    assert.equal(isSameOrSubdomain("deep.r.wxlnk.com", "wxlnk.com"), true);
    assert.equal(isSameOrSubdomain("evilsuffixwxlnk.com", "wxlnk.com"), false);
    assert.equal(isSameOrSubdomain("wxlnk.com.evil", "wxlnk.com"), false);
    assert.equal(isSameOrSubdomain("", "wxlnk.com"), false);
  });

  it("normalizes supported separators, case, trailing dots, and pair order", () => {
    const cases = [
      [" ABC.COM. ", "abc.com"],
      ["abc.com，*.ABC.COM.", "*.abc.com,abc.com"],
      ["abc.com   *.abc.com", "*.abc.com,abc.com"],
      ["abc.com\u0085*.ABC.COM.", "*.abc.com,abc.com"],
      ["  *.abc.com,,，   abc.com  ", "*.abc.com,abc.com"],
      ["XN--BCHER-KVA.EXAMPLE.", "xn--bcher-kva.example"],
    ] as const;

    for (const [input, expected] of cases) {
      assert.equal(normalizeDDNSDomainTargetInput(input), expected, input);
    }
  });

  it("accepts single FQDNs, single wildcards, and exact wildcard/root pairs", () => {
    for (const input of [
      "abc.com",
      "home.abc.com",
      "*.abc.com",
      "xn--bcher-kva.example",
    ]) {
      const result = parseDDNSDomainTargets(input);
      assert.equal(result.ok, true, input);
      if (result.ok) {
        assert.equal(result.pairBase, null, input);
      }
    }

    const pair = parseDDNSDomainTargets("abc.com,*.abc.com");
    assert.deepEqual(pair, {
      ok: true,
      canonical: "*.abc.com,abc.com",
      targets: ["*.abc.com", "abc.com"],
      pairBase: "abc.com",
    });
  });

  it("enforces FQDN and DNS label syntax", () => {
    const valid253 = `${"a".repeat(63)}.${"b".repeat(63)}.${"c".repeat(63)}.${"d".repeat(61)}`;
    const invalid254 = `${valid253}e`;
    assert.equal(valid253.length, 253);
    assert.equal(invalid254.length, 254);
    assert.equal(isValidDDNSDomainTarget(valid253), true);

    for (const input of [
      "localhost",
      "*.com",
      "https://abc.com",
      "abc.com:443",
      "bad_name.abc.com",
      "bücher.example",
      "K.example",
      "abc.com\uFEFF",
      "abc.com\uFEFF*.abc.com",
      "192.0.2.1",
      "192.000.002.001",
      "-home.abc.com",
      "home-.abc.com",
      `${"a".repeat(64)}.abc.com`,
      invalid254,
      "foo*.abc.com",
      "*.*.abc.com",
    ]) {
      const result = parseDDNSDomainTargets(input);
      assert.equal(result.ok, false, input);
      if (!result.ok) {
        assert.equal(result.error, "invalid_domain", input);
      }
    }
  });

  it("rejects every invalid multi-target shape with a stable error", () => {
    const cases = [
      ["abc.com,def.com,ghi.com", "too_many_targets"],
      ["abc.com,abc.com", "duplicate_targets"],
      ["*.abc.com,*.abc.com", "duplicate_targets"],
      ["abc.com,def.com", "invalid_pair"],
      ["*.abc.com,*.def.com", "invalid_pair"],
      ["*.abc.com,abcd.com", "invalid_pair"],
    ] as const;

    for (const [input, expected] of cases) {
      const result = parseDDNSDomainTargets(input);
      assert.equal(result.ok, false, input);
      if (!result.ok) {
        assert.equal(result.error, expected, input);
      }
    }
  });

  it("fails closed for pair capability and validates canonical zone ancestry", () => {
    const pair = "*.abc.com,abc.com";

    for (const capability of [undefined, { mode: "single" as const }]) {
      const result = validateDDNSDomainTargets(pair, { capability });
      assert.equal(result.ok, false);
      if (!result.ok) {
        assert.equal(result.error, "pair_unsupported");
      }
    }

    assert.equal(
      validateDDNSDomainTargets(pair, {
        capability: pairCapability("root_domain"),
        rootDomain: "ABC.COM.",
      }).ok,
      true,
    );

    assert.equal(
      validateDDNSDomainTargets("*.r.wxlnk.com,r.wxlnk.com", {
        capability: pairCapability("root_domain"),
        rootDomain: "WXLNK.COM.",
      }).ok,
      true,
    );

    for (const [pairValue, rootDomain] of [
      [pair, "abcd.com"],
      [pair, "*.abc.com"],
      [pair, "not-a-fqdn"],
      ["*.evilsuffixwxlnk.com,evilsuffixwxlnk.com", "wxlnk.com"],
    ] as const) {
      const result = validateDDNSDomainTargets(pairValue, {
        capability: pairCapability("root_domain"),
        rootDomain,
      });
      assert.equal(result.ok, false, rootDomain);
      if (!result.ok) {
        assert.equal(result.error, "root_mismatch", rootDomain);
      }
    }
  });
});

describe("useDDNSDomainField", () => {
  it("formats on blur/submit and follows the active provider capability", () => {
    const providers = ref([
      provider("pair", pairCapability("root_domain")),
      provider("single", { mode: "single" }),
    ]);
    const providerName = ref("pair");
    const config = ref<Record<string, string>>({
      root_domain: "EXAMPLE.COM.",
      domain: "abc.example.com　*.ABC.EXAMPLE.COM.",
    });
    const includeWildcardHint = ref(true);
    const field = providers.value[0]?.fields[1];
    assert.ok(field);

    const domainField = useDDNSDomainField({
      config,
      includeWildcardHint,
      providerName,
      providers,
      translate: (key) => key,
    });

    domainField.formatOnBlur();
    assert.equal(config.value.domain, "*.abc.example.com,abc.example.com");
    assert.equal(domainField.normalizeForSubmit()?.ok, true);
    assert.match(
      domainField.getFieldDescription(field),
      /admin\.ddns\.domainTargetsPairHint/,
    );
    providerName.value = "single";
    assert.match(
      domainField.getFieldDescription(field),
      /admin\.ddns\.domainTargetsSingleHint/,
    );
    assert.match(
      domainField.getFieldDescription(field),
      /admin\.ddns\.wildcardHint/,
    );
    const singleResult = domainField.validateDomain();
    assert.equal(singleResult?.ok, false);
    if (singleResult && !singleResult.ok) {
      assert.equal(singleResult.error, "pair_unsupported");
    }

    providerName.value = "pair";
    config.value.domain = "abc.com\uFEFF";
    const nonAsciiResult = domainField.normalizeForSubmit();
    assert.equal(nonAsciiResult?.ok, false);
    if (nonAsciiResult && !nonAsciiResult.ok) {
      assert.equal(nonAsciiResult.error, "invalid_domain");
    }
  });
});

describe("DDNS form validation", () => {
  it("validates primary and extra-target domain fields with the same policy", () => {
    const definition = provider("pair", pairCapability("root_domain"));
    const providers = [definition];
    const config = {
      root_domain: "abcd.com",
      domain: "*.abc.com,abc.com",
      update_scope: "ipv4_only",
      ip_source: "public",
    };

    const primaryIssue = validateDDNSCommonConfig({
      config,
      ipSource: "public",
      ipv4Options: [],
      ipv6Options: [],
      providerName: definition.name,
      providers,
      updateScope: "ipv4_only",
    });
    assert.equal(
      primaryIssue?.messageKey,
      "admin.ddns.domainTargetRootMismatch",
    );

    const targetIssue = validateDDNSTargetConfig({
      config,
      ipv4Options: [],
      ipv6Options: [],
      provider: definition.name,
      providerDef: definition,
      providers,
      updateScope: "ipv4_only",
    });
    assert.equal(
      targetIssue?.messageKey,
      "admin.ddns.domainTargetRootMismatch",
    );
  });
});
