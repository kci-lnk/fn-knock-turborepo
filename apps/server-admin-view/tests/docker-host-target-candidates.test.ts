/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ScanDiscoveryHostCandidate } from "../src/lib/api";
import {
  buildDockerHostTargetPlaceholder,
  buildDockerHostTargetSuggestions,
} from "../src/views/subdomain-proxy/useDockerHostTargetCandidates";

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

describe("Docker host target candidates", () => {
  it("uses all Docker host addresses and the recommended placeholder", () => {
    assert.deepEqual(buildDockerHostTargetSuggestions(candidates, true), [
      "192.168.50.8:",
      "10.20.0.8:",
    ]);
    assert.equal(
      buildDockerHostTargetPlaceholder(candidates, true, "LAN_IP:PORT"),
      "10.20.0.8:8080",
    );
  });

  it("never suggests container loopback for Docker without candidates", () => {
    assert.deepEqual(buildDockerHostTargetSuggestions([], true), []);
    assert.equal(
      buildDockerHostTargetPlaceholder([], true, "LAN_IP:PORT"),
      "LAN_IP:PORT",
    );
  });

  it("retains loopback suggestions outside Docker", () => {
    assert.deepEqual(buildDockerHostTargetSuggestions([], false), [
      "127.0.0.1:",
    ]);
    assert.equal(
      buildDockerHostTargetPlaceholder([], false, "unused"),
      "127.0.0.1:5173",
    );
  });
});
