/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  FULL_VERSION_WEBSITE_URL,
  renderReleaseNotesHtml,
  resolveUpdateDetailsAction,
  shouldShowOneClickUpdate,
} from "../src/lib/update-presentation";
import { resolveRuntimeCapabilityRedirect } from "../src/router/runtime-access";
import { privilegedNavigationVisibility } from "../src/views/layout/runtime-navigation";

describe("FPK Lite runtime behavior", () => {
  const liteRouteAccess = {
    canUseTerminal: false,
    canUseSshSecurity: false,
    sshSecurityEnabled: false,
    canUseSmartConnect: false,
    canUseFnosCertificateSync: false,
  };

  it("redirects every privileged route before its page can load", () => {
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect("/terminal", liteRouteAccess),
      { path: "/system" },
    );
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect("/ssh-security", liteRouteAccess),
      { path: "/system", query: { tab: "features" } },
    );
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect(
        "/system/smart-connect",
        liteRouteAccess,
      ),
      { path: "/system", query: { tab: "features" } },
    );
    assert.deepEqual(
      resolveRuntimeCapabilityRedirect(
        "/system/fnos-certificate-sync",
        liteRouteAccess,
      ),
      { path: "/system", query: { tab: "fnos" } },
    );
  });

  it("removes privileged entries from navigation even if restored config enables them", () => {
    assert.deepEqual(
      privilegedNavigationVisibility({
        canUseSshSecurity: false,
        sshSecurityEnabled: true,
        canUseTerminal: false,
        terminalEnabled: true,
      }),
      { sshSecurity: false, terminal: false },
    );
  });

  it("keeps release notes but never exposes one-click update", () => {
    assert.equal(
      shouldShowOneClickUpdate({
        hasUpdate: true,
        canSelfUpdate: true,
        isFpkLite: true,
      }),
      false,
    );
    const releaseNotes = renderReleaseNotesHtml(
      "修复 <安全> [文档](https://docs.fnknock.cn/)",
      "暂无日志",
    );
    assert.match(releaseNotes, /修复 &lt;安全&gt;/u);
    assert.match(releaseNotes, /href="https:\/\/docs\.fnknock\.cn\/"/u);
  });

  it("sends Lite update details to the full-version website", () => {
    assert.deepEqual(resolveUpdateDetailsAction(true), {
      type: "external",
      url: FULL_VERSION_WEBSITE_URL,
    });
  });
});
