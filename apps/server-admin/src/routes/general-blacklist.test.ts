import assert from "node:assert/strict";
import test from "node:test";
import {
  normalizeGeneralBlacklistSource,
  normalizeGeneralBlacklistIpList,
  normalizeGeneralBlacklistStatusIpList,
  parseGeneralBlacklistDeleteIps,
} from "./general-blacklist";

test("general blacklist IP parser normalizes and deduplicates valid IPs", () => {
  assert.deepEqual(
    normalizeGeneralBlacklistIpList([
      "203.0.113.10",
      "203.0.113.10:443",
      "[2001:db8::10]",
      "2001:db8::10",
    ]),
    ["203.0.113.10", "2001:db8::10"],
  );
});

test("general blacklist IP parser rejects mixed invalid IPs", () => {
  assert.throws(
    () => normalizeGeneralBlacklistIpList(["203.0.113.10", "bad-ip"]),
    /Invalid IP: bad-ip/,
  );
  assert.throws(
    () => normalizeGeneralBlacklistIpList(["203.0.113.10", ""]),
    /Invalid IP/,
  );
  assert.throws(
    () => normalizeGeneralBlacklistIpList(["203.0.113.10", 42]),
    /Invalid IP/,
  );
});

test("general blacklist status parser ignores invalid members", () => {
  assert.deepEqual(
    normalizeGeneralBlacklistStatusIpList([
      "203.0.113.10",
      "bad-ip",
      "",
      42,
      "[2001:db8::10]",
    ]),
    ["203.0.113.10", "2001:db8::10"],
  );
});

test("general blacklist source parser accepts waf log source", () => {
  assert.equal(normalizeGeneralBlacklistSource("waf_log"), "waf_log");
  assert.equal(normalizeGeneralBlacklistSource("unknown"), "manual");
});

test("general blacklist delete parser rejects invalid members", () => {
  assert.throws(
    () => parseGeneralBlacklistDeleteIps({ ips: ["203.0.113.10", "bad-ip"] }),
    /Invalid IP: bad-ip/,
  );
});
