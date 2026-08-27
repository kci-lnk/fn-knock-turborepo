/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ScanDiscoveryHostCandidate } from "../src/lib/api";
import {
  buildHostTargetPlaceholder,
  buildHostTargetSuggestions,
} from "../src/views/subdomain-proxy/useHostTargetCandidates";

const candidates: ScanDiscoveryHostCandidate[] = [
  {
    address: "192.168.50.8",
    cidr: "192.168.50.0/24",
    source: "proxy",
    recommended: false,
    includedInAutomaticScan: true,
  },
  {
    address: "10.20.0.8",
    cidr: "10.20.0.0/23",
    source: "configured",
    recommended: true,
    includedInAutomaticScan: true,
  },
];

describe("Host target candidates", () => {
  it("uses all Docker host addresses and the recommended placeholder", () => {
    assert.deepEqual(buildHostTargetSuggestions(candidates, true), [
      "192.168.50.8:",
      "10.20.0.8:",
    ]);
    assert.equal(
      buildHostTargetPlaceholder(candidates, true, "LAN_IP:PORT"),
      "10.20.0.8:8080",
    );
  });

  it("never suggests container loopback for Docker without candidates", () => {
    assert.deepEqual(buildHostTargetSuggestions([], true), []);
    assert.equal(
      buildHostTargetPlaceholder([], true, "LAN_IP:PORT"),
      "LAN_IP:PORT",
    );
  });

  it("filters stale native candidates while Docker candidates reload", () => {
    const staleCandidates: ScanDiscoveryHostCandidate[] = [
      {
        address: "127.0.0.1",
        cidr: "127.0.0.1/32",
        source: "loopback",
        recommended: true,
        includedInAutomaticScan: true,
      },
      {
        address: "172.17.0.2",
        cidr: "172.17.0.0/24",
        source: "interface",
        recommended: false,
        includedInAutomaticScan: true,
      },
      candidates[0],
    ];
    assert.deepEqual(buildHostTargetSuggestions(staleCandidates, true), [
      "192.168.50.8:",
    ]);
  });

  it("uses loopback and all detected LAN addresses outside Docker", () => {
    const nativeCandidates: ScanDiscoveryHostCandidate[] = [
      {
        address: "127.0.0.1",
        cidr: "127.0.0.1/32",
        source: "loopback",
        recommended: true,
        includedInAutomaticScan: true,
      },
      {
        address: "192.168.50.8",
        cidr: "192.168.50.0/24",
        source: "interface",
        recommended: false,
        includedInAutomaticScan: true,
      },
      {
        address: "10.20.0.8",
        cidr: "10.20.0.0/24",
        source: "interface",
        recommended: false,
        includedInAutomaticScan: true,
      },
    ];
    assert.deepEqual(buildHostTargetSuggestions(nativeCandidates, false), [
      "127.0.0.1:",
      "192.168.50.8:",
      "10.20.0.8:",
    ]);
    assert.equal(
      buildHostTargetPlaceholder(nativeCandidates, false, "unused"),
      "127.0.0.1:5173",
    );
  });

  it("falls back to native loopback when loading candidates fails", () => {
    assert.deepEqual(buildHostTargetSuggestions([], false), ["127.0.0.1:"]);
    assert.equal(
      buildHostTargetPlaceholder([], false, "unused"),
      "127.0.0.1:5173",
    );
  });

  it("keeps native loopback first when an older response omits it", () => {
    const interfaceCandidate: ScanDiscoveryHostCandidate = {
      ...candidates[0],
      source: "interface",
    };
    assert.deepEqual(buildHostTargetSuggestions([interfaceCandidate], false), [
      "127.0.0.1:",
      "192.168.50.8:",
    ]);
    assert.equal(
      buildHostTargetPlaceholder([interfaceCandidate], false, "unused"),
      "127.0.0.1:5173",
    );
  });

  it("filters stale Docker candidates while native candidates reload", () => {
    assert.deepEqual(buildHostTargetSuggestions(candidates, false), [
      "127.0.0.1:",
    ]);
  });

  it("deduplicates repeated addresses without changing priority", () => {
    assert.deepEqual(
      buildHostTargetSuggestions([candidates[0], candidates[0]], true),
      ["192.168.50.8:"],
    );
  });
});
