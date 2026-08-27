/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { supportsSharedBackupForRuntime } from "../src/lib/maintenance-runtime";
import type { RuntimeCapabilities, RuntimeProfile } from "../src/types";

const profile = (
  deployment_target: RuntimeProfile["deployment_target"],
): RuntimeProfile => ({
  deployment_target,
  is_docker: deployment_target === "docker",
  is_linux: true,
  is_windows: deployment_target === "windows",
  is_root_process: true,
});

const capabilities = (
  patch: Partial<RuntimeCapabilities> = {},
): RuntimeCapabilities => ({
  direct_mode_available: true,
  host_firewall_available: true,
  smart_connect_available: true,
  system_clock_sync_available: true,
  self_update_available: false,
  shared_root_available: false,
  ...patch,
});

describe("maintenance runtime helpers", () => {
  it("shows shared backup actions for FPK even before shared root is confirmed", () => {
    assert.equal(
      supportsSharedBackupForRuntime(profile("fpk"), capabilities()),
      true,
    );
  });

  it("shows shared backup actions for the isolated FPK Lite data share", () => {
    assert.equal(
      supportsSharedBackupForRuntime(profile("fpk-lite"), capabilities()),
      true,
    );
  });

  it("shows shared backup actions when FPK capability is present without a profile", () => {
    assert.equal(
      supportsSharedBackupForRuntime(
        undefined,
        capabilities({ self_update_available: true }),
      ),
      true,
    );
  });

  it("shows shared backup actions when a shared root is available", () => {
    assert.equal(
      supportsSharedBackupForRuntime(
        profile("dev"),
        capabilities({ shared_root_available: true }),
      ),
      true,
    );
  });

  it("hides shared backup actions for Docker and OpenWrt", () => {
    assert.equal(
      supportsSharedBackupForRuntime(
        profile("docker"),
        capabilities({ shared_root_available: true }),
      ),
      false,
    );
    assert.equal(
      supportsSharedBackupForRuntime(
        profile("openwrt"),
        capabilities({ shared_root_available: true }),
      ),
      false,
    );
  });
});
