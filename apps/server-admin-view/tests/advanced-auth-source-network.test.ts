/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  formatAdvancedAuthValueList,
  getSourceNetworkValidationIssue,
  parseAdvancedAuthValueList,
  parseSourceNetworkTextarea,
} from "../src/views/subdomain-proxy/advanced-auth-source-network";

describe("advanced authentication source networks", () => {
  it("parses, trims, and deduplicates compact IPv4 and IPv6 values", () => {
    assert.deepEqual(
      parseSourceNetworkTextarea(
        " 192.0.2.10 \r\n2001:db8::10\n192.0.2.10，2001:DB8::10",
      ),
      ["192.0.2.10", "2001:db8::10"],
    );
  });

  it("parses comma-separated values without losing quoted commas", () => {
    assert.deepEqual(
      parseAdvancedAuthValueList(
        ' alpha, "contains,comma"，"say ""hello"""\n beta ',
      ),
      ["alpha", "contains,comma", 'say "hello"', "beta"],
    );
  });

  it("formats and parses compact values without changing their contents", () => {
    const values = [
      "plain",
      "contains,comma",
      "包含，逗号",
      'say "hello"',
      " padded ",
    ];
    assert.deepEqual(
      parseAdvancedAuthValueList(formatAdvancedAuthValueList(values)),
      values,
    );
  });

  it("accepts multiple IPv4 and IPv6 addresses for exact operators", () => {
    assert.equal(
      getSourceNetworkValidationIssue(["192.0.2.10", "2001:db8::10"], "equals"),
      null,
    );
    assert.deepEqual(
      getSourceNetworkValidationIssue(
        ["192.0.2.10", "2001:db8::/32"],
        "not_equals",
      ),
      { kind: "address", line: 2 },
    );
    assert.deepEqual(
      getSourceNetworkValidationIssue(
        ["2001:db8::10", "2001:db8:::11"],
        "equals",
      ),
      { kind: "address", line: 2 },
    );
  });

  it("accepts multiple IPv4 and IPv6 CIDRs and rejects bare addresses", () => {
    assert.equal(
      getSourceNetworkValidationIssue(
        ["192.0.2.0/24", "2001:db8::/32"],
        "in_cidr",
      ),
      null,
    );
    assert.deepEqual(
      getSourceNetworkValidationIssue(
        ["192.0.2.0/24", "2001:db8::10"],
        "not_in_cidr",
      ),
      { kind: "cidr", line: 2 },
    );
    assert.deepEqual(
      getSourceNetworkValidationIssue(["::/0", "2001:db8::/129"], "in_cidr"),
      { kind: "cidr", line: 2 },
    );
  });
});
