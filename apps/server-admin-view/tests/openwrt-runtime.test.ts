/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { resolveRuntimeCapabilityRedirect } from "../src/router/runtime-access";
import {
  privilegedNavigationVisibility,
  smartConnectFeatureEntryVisible,
} from "../src/views/layout/runtime-navigation";

describe("OpenWrt runtime behavior", () => {
  const openWrtRouteAccess = {
    canUseTerminal: false,
    canUseSshSecurity: false,
    sshSecurityEnabled: false,
    canUseSmartConnect: false,
    canUseFnosCertificateSync: false,
  };

  it("redirects unsupported OpenWrt feature routes", () => {
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect("/terminal", openWrtRouteAccess),
      { path: "/system" },
    );
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect("/ssh-security", openWrtRouteAccess),
      { path: "/system", query: { tab: "features" } },
    );
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect(
        "/system/smart-connect",
        openWrtRouteAccess,
      ),
      { path: "/system", query: { tab: "features" } },
    );
  });

  it("hides SSH security, Web terminal, and Smart Connect entries", () => {
    assert.deepEqual(
      privilegedNavigationVisibility({
        canUseSshSecurity: false,
        sshSecurityEnabled: true,
        canUseTerminal: false,
        terminalEnabled: true,
      }),
      { sshSecurity: false, terminal: false },
    );
    assert.equal(
      smartConnectFeatureEntryVisible({
        isFpkLiteDeployment: false,
        isDockerDeployment: false,
        isOpenWrtDeployment: true,
        isSynologyDeployment: false,
      }),
      false,
    );
  });
});
