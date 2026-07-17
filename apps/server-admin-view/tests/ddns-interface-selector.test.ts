import assert from "node:assert/strict";
import test from "node:test";
import type { DDNSNetworkInterfacePayload } from "../src/lib/api/ddns";
import {
  INTERFACE_IPV6_SELECTOR_KEY,
  IP_SOURCE_KEY,
  NETWORK_INTERFACE_KEY,
  UPDATE_SCOPE_KEY,
  buildInterfaceSelectorFromLegacyIndex,
  extractCommonTargetConfig,
  ipv6InterfaceIdFromAddress,
  parseInterfaceSelector,
  serializeInterfaceSelector,
  validateDDNSCommonConfig,
} from "../src/views/ddns-management/model";

const networkInterface: DDNSNetworkInterfacePayload = {
  name: "eth0",
  label: "eth0",
  summary: "IPv6",
  hasIpv4: false,
  hasIpv6: true,
  addresses: [],
  selectableAddresses: [
    {
      family: "ipv6",
      address: "2001:db8:1::1234",
      cidr: "2001:db8:1::1234/64",
      internal: false,
      temporary: false,
    },
  ],
};

test("extracts a canonical IPv6 lower-64-bit interface ID", () => {
  assert.equal(
    ipv6InterfaceIdFromAddress("2001:db8:1::1234"),
    "0000:0000:0000:1234",
  );
});

test("migrates a legacy IPv6 index to a soft preferred-address selector", () => {
  const result = buildInterfaceSelectorFromLegacyIndex(
    networkInterface,
    "ipv6",
    "0",
  );
  assert.equal(result.migrated, true);
  assert.equal(result.selector.mode, "auto");
  assert.equal(result.selector.preferredAddress, "2001:db8:1::1234");
  assert.equal(result.selector.ipv6InterfaceId, undefined);
  assert.equal(result.selector.allowTemporary, false);
});

test("temporary legacy addresses retain explicit opt-in during migration", () => {
  const option = structuredClone(networkInterface);
  option.selectableAddresses[0]!.temporary = true;
  const result = buildInterfaceSelectorFromLegacyIndex(option, "ipv6", "0");
  assert.equal(result.selector.mode, "auto");
  assert.equal(result.selector.allowTemporary, true);
  assert.equal(result.selector.ipv6InterfaceId, undefined);
});

test("unknown IPv6 status does not assume the interface ID is stable", () => {
  const option = structuredClone(networkInterface);
  delete option.selectableAddresses[0]!.temporary;
  const result = buildInterfaceSelectorFromLegacyIndex(option, "ipv6", "0");
  assert.equal(result.selector.mode, "auto");
  assert.equal(result.selector.ipv6InterfaceId, undefined);
});

test("does not migrate an unresolved legacy index", () => {
  const result = buildInterfaceSelectorFromLegacyIndex(
    networkInterface,
    "ipv6",
    "3",
  );
  assert.equal(result.migrated, false);
  assert.equal(result.selector.preferredAddress, undefined);
});

test("does not treat a missing legacy index as index zero", () => {
  const result = buildInterfaceSelectorFromLegacyIndex(
    networkInterface,
    "ipv6",
    "",
  );
  assert.equal(result.migrated, false);
  assert.equal(result.selector.preferredAddress, undefined);
});

test("legacy migration recovers the published address before using its index", () => {
  const option = structuredClone(networkInterface);
  option.selectableAddresses.unshift({
    ...option.selectableAddresses[0]!,
    address: "2001:db8:1::1000",
  });
  const result = buildInterfaceSelectorFromLegacyIndex(
    option,
    "ipv6",
    "0",
    "2001:db8:1::1234",
  );
  assert.equal(result.selector.preferredAddress, "2001:db8:1::1234");
});

test("legacy migration follows a changed prefix when the interface ID matches", () => {
  const result = buildInterfaceSelectorFromLegacyIndex(
    networkInterface,
    "ipv6",
    "3",
    "2001:db8:ffff::1234",
  );
  assert.equal(result.migrated, true);
  assert.equal(result.selector.preferredAddress, "2001:db8:1::1234");
  assert.equal(result.selector.ipv6InterfaceId, undefined);
});

test("parses selector JSON and rejects unsupported versions", () => {
  const selector = buildInterfaceSelectorFromLegacyIndex(
    networkInterface,
    "ipv6",
    "0",
  ).selector;
  assert.deepEqual(
    parseInterfaceSelector(serializeInterfaceSelector(selector)),
    selector,
  );
  assert.equal(
    parseInterfaceSelector(
      JSON.stringify({ version: 2, mode: "auto", allowTemporary: false }),
    ),
    null,
  );
});

test("interface validation accepts a semantic selector without an index", () => {
  const selector = buildInterfaceSelectorFromLegacyIndex(
    networkInterface,
    "ipv6",
    "0",
  ).selector;
  const issue = validateDDNSCommonConfig({
    config: {
      [IP_SOURCE_KEY]: "interface",
      [NETWORK_INTERFACE_KEY]: "eth0",
      [UPDATE_SCOPE_KEY]: "ipv6_only",
      [INTERFACE_IPV6_SELECTOR_KEY]: serializeInterfaceSelector(selector),
    },
    ipSource: "interface",
    ipv4Options: [],
    ipv6Options: [{ value: "0", label: "IPv6" }],
    providerName: "cloudflare",
    providers: [],
    updateScope: "ipv6_only",
  });
  assert.equal(issue, null);
});

test("interface validation allows implicit automatic IPv6 selection", () => {
  const issue = validateDDNSCommonConfig({
    config: {
      [IP_SOURCE_KEY]: "interface",
      [NETWORK_INTERFACE_KEY]: "eth0",
      [UPDATE_SCOPE_KEY]: "ipv6_only",
    },
    ipSource: "interface",
    ipv4Options: [],
    ipv6Options: [{ value: "0", label: "IPv6" }],
    providerName: "cloudflare",
    providers: [],
    updateScope: "ipv6_only",
  });
  assert.equal(issue, null);
});

test("interface validation still rejects malformed explicit selectors", () => {
  const issue = validateDDNSCommonConfig({
    config: {
      [IP_SOURCE_KEY]: "interface",
      [NETWORK_INTERFACE_KEY]: "eth0",
      [UPDATE_SCOPE_KEY]: "ipv6_only",
      [INTERFACE_IPV6_SELECTOR_KEY]: "{invalid",
    },
    ipSource: "interface",
    ipv4Options: [],
    ipv6Options: [{ value: "0", label: "IPv6" }],
    providerName: "cloudflare",
    providers: [],
    updateScope: "ipv6_only",
  });
  assert.equal(issue?.messageKey, "admin.ddns.interfaceSelectorInvalid");
});

test("loaded target normalization preserves the IPv6 selector", () => {
  const selector = serializeInterfaceSelector({
    version: 1,
    mode: "rules",
    includeCidrs: ["2409:8a74::/32"],
    allowTemporary: false,
  });
  const normalized = extractCommonTargetConfig({
    [INTERFACE_IPV6_SELECTOR_KEY]: selector,
  });

  assert.equal(normalized[INTERFACE_IPV6_SELECTOR_KEY], selector);
});
