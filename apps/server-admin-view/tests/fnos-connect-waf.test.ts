/// <reference types="node" />

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";

import { canUseFnosConnectWafForRuntime } from "../src/lib/fnos-connect-waf";
import type { RuntimeCapabilities, RuntimeProfile } from "../src/types";

const profile = (deployment_target: RuntimeProfile["deployment_target"]) =>
  ({
    deployment_target,
    is_docker: deployment_target === "docker",
    is_linux: true,
    is_windows: false,
    is_root_process: true,
  }) satisfies RuntimeProfile;

const capabilities = (available: boolean) =>
  ({
    direct_mode_available: false,
    host_firewall_available: false,
    smart_connect_available: false,
    system_clock_sync_available: false,
    self_update_available: false,
    terminal_available: false,
    shared_root_available: false,
    fnos_connect_waf_available: available,
  }) satisfies RuntimeCapabilities;

describe("FN Connect WAF visibility", () => {
  it("is exposed only when standard FPK and the backend capability both agree", () => {
    assert.equal(
      canUseFnosConnectWafForRuntime(profile("fpk"), capabilities(true)),
      true,
    );
    for (const target of [
      "fpk-lite",
      "docker",
      "linux",
      "openwrt",
      "synology",
      "windows",
      "dev",
    ] as const) {
      assert.equal(
        canUseFnosConnectWafForRuntime(profile(target), capabilities(true)),
        false,
      );
    }
    assert.equal(
      canUseFnosConnectWafForRuntime(profile("fpk"), capabilities(false)),
      false,
    );
    assert.equal(
      canUseFnosConnectWafForRuntime(profile("fpk"), undefined),
      false,
    );
  });

  it("guards component creation so Lite never calls the endpoint", async () => {
    const source = await readFile(
      new URL("../src/views/system-settings/FnosSettings.vue", import.meta.url),
      "utf8",
    );
    assert.match(
      source,
      /<FnosConnectWafSetting v-if="canUseFnosConnectWaf" \/>/u,
    );
  });
});
