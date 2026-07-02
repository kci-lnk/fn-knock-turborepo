import assert from "node:assert/strict";
import test from "node:test";
import {
  WhitelistRegionValidationError,
  normalizeWhitelistRegionInputs,
  resolveWhitelistRegionCidrs,
  type WhitelistRegionLookup,
} from "./whitelist-regions";

test("whitelist region parser normalizes and deduplicates region inputs", () => {
  assert.deepEqual(
    normalizeWhitelistRegionInputs([
      { province: " 广东 ", query_city: " 深圳 " },
      { province: "广东", query_city: "深圳" },
      { province: "浙江", query_city: null },
      { province: "" },
      null,
    ]),
    [
      { province: "广东", query_city: "深圳" },
      { province: "浙江", query_city: null },
    ],
  );
});

test("whitelist region add rejects empty region input", async () => {
  await assert.rejects(
    () =>
      resolveWhitelistRegionCidrs({
        regions: [],
        lookupCidrs: async () => ({ cidrGroups: { ipv4: [], ipv6: [] } }),
      }),
    WhitelistRegionValidationError,
  );
});

test("whitelist region resolver deduplicates CIDRs", async () => {
  const lookupCalls: Array<{ province: string; city?: string | null }> = [];
  const lookupCidrs: WhitelistRegionLookup = async (input) => {
    lookupCalls.push(input);
    return {
      cidrGroups: {
        ipv4: ["203.0.113.42/24", "203.0.113.0/24"],
        ipv6: ["2001:db8::/32"],
      },
    };
  };

  const result = await resolveWhitelistRegionCidrs({
    regions: [
      { province: "广东", query_city: "深圳" },
      { province: "广东", query_city: "深圳" },
    ],
    lookupCidrs,
  });

  assert.deepEqual(lookupCalls, [{ province: "广东", city: "深圳" }]);
  assert.deepEqual(result.cidrs, ["203.0.113.0/24", "2001:db8::/32"]);
  assert.equal(result.total, 2);
});

test("whitelist region resolver rejects regions without resolved CIDRs", async () => {
  await assert.rejects(
    () =>
      resolveWhitelistRegionCidrs({
        regions: [{ province: "广东", query_city: "深圳" }],
        lookupCidrs: async () => ({ cidrGroups: { ipv4: [], ipv6: [] } }),
      }),
    WhitelistRegionValidationError,
  );
});

test("whitelist region resolver propagates CIDR lookup failures", async () => {
  await assert.rejects(
    () =>
      resolveWhitelistRegionCidrs({
        regions: [{ province: "广东", query_city: "深圳" }],
        lookupCidrs: async () => {
          throw new Error("cidr service unavailable");
        },
      }),
    /cidr service unavailable/,
  );
});
