import assert from "node:assert/strict";
import test from "node:test";
import { buildInterfaceIpv4Cidr } from "./scan-discovery";

test("interface discovery uses the real interface prefix when it fits the scan host limit", () => {
  assert.equal(buildInterfaceIpv4Cidr("192.168.30.42", 23), "192.168.30.0/23");
});

test("interface discovery falls back to /24 when the real prefix is too broad", () => {
  assert.equal(buildInterfaceIpv4Cidr("192.168.31.42", 20), "192.168.31.0/24");
});
