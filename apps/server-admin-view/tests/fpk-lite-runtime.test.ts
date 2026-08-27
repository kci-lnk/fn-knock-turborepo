/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  FULL_VERSION_WEBSITE_URL,
  OFFICIAL_DOCUMENTATION_URL,
  OFFICIAL_WEBSITE_URL,
  resolveUpdateDetailsAction,
  shouldShowOneClickUpdate,
} from "../src/lib/update-presentation";
import { renderReleaseNotesHtml } from "../src/lib/release-notes";
import { resolveRuntimeCapabilityRedirect } from "../src/router/runtime-access";
import { privilegedNavigationVisibility } from "../src/views/layout/runtime-navigation";

describe("FPK Lite runtime behavior", () => {
  const liteRouteAccess = {
    canUseSshSecurity: false,
    sshSecurityEnabled: false,
    canUseSmartConnect: false,
    canUseFnosCertificateSync: false,
  };

  it("keeps the SSH terminal route while redirecting host-privileged routes", () => {
    assert.equal(
      resolveRuntimeCapabilityRedirect("/terminal", liteRouteAccess),
      null,
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

  it("removes unavailable SSH security from navigation", () => {
    assert.deepEqual(
      privilegedNavigationVisibility({
        canUseSshSecurity: false,
        sshSecurityEnabled: true,
      }),
      { sshSecurity: false },
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
      [
        "[**用户协议与隐私政策**](https://www.fnknock.cn/legal)",
        "",
        "# fn-knock 2.1.6",
        "",
        "> [!IMPORTANT]",
        "> **这是一个紧急修复版本。** 建议尽快升级。",
        "",
        "- 修复 <安全>",
        "- 查看 [文档](https://docs.fnknock.cn/)",
        "",
        "---",
      ].join("\n"),
      "暂无日志",
    );
    assert.match(releaseNotes, /<h4>fn-knock 2\.1\.6<\/h4>/u);
    assert.match(
      releaseNotes,
      /<aside class="release-note-alert release-note-alert--important" aria-label="Important">/u,
    );
    assert.match(
      releaseNotes,
      /<p class="release-note-alert__title">Important<\/p>/u,
    );
    assert.match(
      releaseNotes,
      /<p><strong>这是一个紧急修复版本。<\/strong> 建议尽快升级。<\/p>/u,
    );
    assert.doesNotMatch(releaseNotes, /&gt; \[!IMPORTANT\]/u);
    assert.match(releaseNotes, /<ul><li>修复 &lt;安全&gt;<\/li>/u);
    assert.match(releaseNotes, /<hr>/u);
    assert.match(
      releaseNotes,
      /<a href="https:\/\/www\.fnknock\.cn\/legal"[^>]*><strong>用户协议与隐私政策<\/strong><\/a>/u,
    );
    assert.match(releaseNotes, /href="https:\/\/docs\.fnknock\.cn\/"/u);
  });

  it("escapes raw HTML and leaves unsupported links and images as plain text", () => {
    const releaseNotes = renderReleaseNotesHtml(
      [
        "<script>alert('xss')</script>",
        "[危险链接](javascript:alert('xss'))",
        "![远程图片](https://example.com/tracker.png)",
      ].join("\n\n"),
      "暂无日志",
    );

    assert.doesNotMatch(releaseNotes, /<script>/u);
    assert.match(releaseNotes, /&lt;script&gt;/u);
    assert.doesNotMatch(releaseNotes, /href=/u);
    assert.doesNotMatch(releaseNotes, /<img/u);
  });

  it("sends Lite update details to the full-version website", () => {
    assert.deepEqual(resolveUpdateDetailsAction(true), {
      type: "external",
      url: FULL_VERSION_WEBSITE_URL,
    });
  });

  it("keeps stable official website and documentation links", () => {
    assert.equal(OFFICIAL_WEBSITE_URL, "https://www.fnknock.cn/");
    assert.equal(OFFICIAL_DOCUMENTATION_URL, "https://docs.fnknock.cn/");
  });
});
