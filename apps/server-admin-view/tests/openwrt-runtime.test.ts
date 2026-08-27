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
    canUseSshSecurity: false,
    sshSecurityEnabled: false,
    canUseSmartConnect: false,
    canUseFnosCertificateSync: false,
  };

  it("keeps the SSH terminal route while redirecting unsupported host features", () => {
    assert.equal(
      resolveRuntimeCapabilityRedirect("/terminal", openWrtRouteAccess),
      null,
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

  it("hides SSH security and Smart Connect entries", () => {
    assert.deepEqual(
      privilegedNavigationVisibility({
        canUseSshSecurity: false,
        sshSecurityEnabled: true,
      }),
      { sshSecurity: false },
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
