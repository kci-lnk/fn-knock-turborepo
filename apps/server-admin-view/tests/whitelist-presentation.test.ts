import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { WhiteListRecord } from "../src/lib/api/whitelist";
import {
  formatWhitelistRemaining,
  getWhitelistResolveStatusLabel,
  getWhitelistResolveStatusVariant,
  getWhitelistTargetTypeLabel,
  type WhitelistTranslate,
} from "../src/views/ip-whitelist/whitelistPresentation";

const recordWithStatus = (resolveStatus: WhiteListRecord["resolveStatus"]) =>
  ({ resolveStatus }) as WhiteListRecord;

describe("IP whitelist presentation", () => {
  it("maps target and resolver states to stable UI tokens", () => {
    const translate: WhitelistTranslate = (key) => key;
    assert.equal(getWhitelistTargetTypeLabel("ip"), "IP");
    assert.equal(getWhitelistTargetTypeLabel("cidr"), "CIDR");
    assert.equal(getWhitelistTargetTypeLabel("cname"), "CNAME");
    assert.equal(
      getWhitelistResolveStatusLabel(recordWithStatus("resolved"), translate),
      "admin.ipWhitelist.resolveSuccess",
    );
    assert.equal(
      getWhitelistResolveStatusVariant(recordWithStatus("error")),
      "destructive",
    );
    assert.equal(
      getWhitelistResolveStatusVariant(recordWithStatus(undefined)),
      "outline",
    );
  });

  it("formats expiry durations with a deterministic clock", () => {
    const translate: WhitelistTranslate = (key, params) => {
      if (key.endsWith(".days")) return `${params?.count}d`;
      if (key.endsWith(".hours")) return `${params?.count}h`;
      if (key.endsWith(".minutesCount")) return `${params?.count}m`;
      if (key.endsWith(".remaining")) return `${params?.value} left`;
      return key;
    };
    assert.equal(formatWhitelistRemaining(90_060, translate, 0), "1d1h1m left");
    assert.equal(
      formatWhitelistRemaining(100, translate, 100),
      "admin.ipWhitelist.expired",
    );
  });
});
