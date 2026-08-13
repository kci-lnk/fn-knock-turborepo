import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { describe, it } from "node:test";

const readSource = (relativePath: string) =>
  readFileSync(new URL(relativePath, import.meta.url), "utf8");

const collectVueFiles = (relativeRoot: string): string[] => {
  const root = new URL(relativeRoot, import.meta.url);
  const walk = (directory: string): string[] =>
    readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
      const absolutePath = path.join(directory, entry.name);
      if (entry.isDirectory()) return walk(absolutePath);
      return entry.name.endsWith(".vue") ? [absolutePath] : [];
    });
  return walk(root.pathname);
};

describe("interactive affordance contract", () => {
  it("keeps shared buttons visibly interactive without relying on hover", () => {
    const source = readSource(
      "../../../packages/ui-vue/src/components/ui/button/index.ts",
    );

    assert.match(source, /inline-flex cursor-pointer/u);
    assert.match(source, /"destructive-outline":/u);
    assert.match(source, /border border-destructive\/35/u);
    assert.match(
      source,
      /link:\s*\n?\s*"text-primary underline decoration-primary\/55/u,
    );

    for (const stylesheet of [
      "../src/assets/index.css",
      "../../server-auth-view/src/assets/index.css",
    ]) {
      const css = readSource(stylesheet);
      assert.match(css, /button:not\(:disabled\)/u);
      assert.match(css, /cursor: pointer/u);
    }
  });

  it("marks help, copy, edit, and detail triggers with stable cues", () => {
    const fixtures = [
      [
        "../../../packages/admin-shared/src/components/common/OverflowTooltipText.vue",
        [/data-affordance="help"/u, /decoration-dotted/u],
      ],
      [
        "../src/views/ddns-management/DDNSStatusCard.vue",
        [/copyAddressAria/u, /@click="copyIpAddress/u],
      ],
      [
        "../src/views/ddns-management/DDNSExtraTargetsCard.vue",
        [/copyAddressAria/u, /@click="copyIpAddress/u],
      ],
      [
        "../src/views/subdomain-proxy/SubdomainMappingTitleCell.vue",
        [/data-affordance="edit"/u, /<Pencil/u],
      ],
      [
        "../src/components/HostTrafficActivity.vue",
        [/admin\.hostTraffic\.view/u, /hover:bg-muted\/50/u],
      ],
      [
        "../src/views/auth-settings/TotpCredentialTable.vue",
        [
          /data-affordance="details"/u,
          /data-affordance="edit"/u,
          /data-affordance="help"/u,
        ],
      ],
    ] as const;

    for (const [relativePath, patterns] of fixtures) {
      const source = readSource(relativePath);
      for (const pattern of patterns) {
        assert.match(source, pattern, `${relativePath} must match ${pattern}`);
      }
    }

    const subdomainMappingsSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingTableRow.vue",
    );
    assert.doesNotMatch(
      subdomainMappingsSource,
      /<Copy\b/u,
      "domain rows must not render a copy icon",
    );
  });

  it("keeps inline copy values iconless", () => {
    for (const relativePath of [
      "../src/views/subdomain-proxy/SubdomainMappingTableRow.vue",
      "../src/views/ddns-management/DDNSStatusCard.vue",
      "../src/views/ddns-management/DDNSExtraTargetsCard.vue",
      "../src/views/OIDCProviderSettings.vue",
    ]) {
      const source = readSource(relativePath);
      assert.doesNotMatch(source, /<Copy\b/u, relativePath);
    }
  });

  it("keeps the traffic details trigger in its compact text form", () => {
    const source = readSource("../src/components/HostTrafficActivity.vue");
    assert.match(source, /inline-flex min-h-6[^\n"]*px-1\.5/u);
    assert.doesNotMatch(
      source,
      /data-affordance="details"|<Activity\b|border-border\/60/u,
    );
  });

  it("reveals inline edit icons on hover or keyboard focus", () => {
    const inlineEditorSource = readSource(
      "../../../packages/admin-shared/src/components/InlineCommentEditor.vue",
    );
    assert.match(
      inlineEditorSource,
      /pointer-events-none[^\n"]*opacity-0[^\n"]*group-hover:pointer-events-auto[^\n"]*group-hover:opacity-100/u,
    );
    assert.match(
      inlineEditorSource,
      /group-focus-within:pointer-events-auto[^\n"]*group-focus-within:opacity-100/u,
    );

    const mappingSource = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingTitleCell.vue",
    );
    assert.match(mappingSource, /class="group\/edit inline-flex/u);
    assert.match(
      mappingSource,
      /<Pencil\s+class="[^"]*opacity-0[^"]*group-hover\/edit:opacity-100[^"]*group-focus-visible\/edit:opacity-100/u,
    );
  });

  it("keeps relative time chips aligned without an underline", () => {
    const timeSource = readSource(
      "../../../packages/admin-shared/src/components/common/HumanFriendlyTime.vue",
    );
    assert.match(
      timeSource,
      /inline-flex items-center justify-center align-middle/u,
    );
    assert.match(timeSource, /\[line-height:inherit\]/u);
    assert.doesNotMatch(
      timeSource,
      /data-affordance="help"|decoration-dotted|underline-offset/u,
    );

    for (const relativePath of [
      "../src/views/gateway-request-logs/GatewayRequestLogDesktopRow.vue",
      "../src/views/waf-logs/WAFLogsTable.vue",
      "../src/views/event-center/EventsTab.vue",
    ]) {
      const source = readSource(relativePath);
      assert.match(source, /flex items-center gap-2/u, relativePath);
      assert.match(
        source,
        /inline-flex h-5 shrink-0 items-center rounded-full/u,
        relativePath,
      );
    }
  });

  it("only hides the approved inline edit affordances until hover", () => {
    const files = [
      ...collectVueFiles("../src"),
      ...collectVueFiles("../../../packages/admin-shared/src"),
    ];
    const approved = new Set([
      new URL(
        "../src/views/subdomain-proxy/SubdomainMappingTitleCell.vue",
        import.meta.url,
      ).pathname,
      new URL(
        "../../../packages/admin-shared/src/components/InlineCommentEditor.vue",
        import.meta.url,
      ).pathname,
    ]);

    for (const file of files) {
      const source = readFileSync(file, "utf8");
      const hidesUntilHover =
        /(?:invisible\s+)?opacity-0[^\n"]*group-hover/u.test(source);
      assert.equal(
        hidesUntilHover,
        approved.has(file),
        `${file} has an unexpected hover-only affordance contract`,
      );
    }
  });

  it("does not use text-only ghost buttons as standalone actions", () => {
    const files = [
      ...collectVueFiles("../src"),
      ...collectVueFiles("../../../packages/admin-shared/src"),
    ];
    const violations: string[] = [];

    for (const file of files) {
      const source = readFileSync(file, "utf8");
      for (const match of source.matchAll(
        /<Button\b([\s\S]*?)>([\s\S]*?)<\/Button>/gu,
      )) {
        const attributes = match[1].replace(/\s+/gu, " ");
        if (!attributes.includes('variant="ghost"')) continue;
        if (
          /size="icon/u.test(attributes) ||
          /(border|bg-|floating-cursor|rounded-xl)/u.test(attributes)
        ) {
          continue;
        }
        const body = match[2];
        const hasVisualIcon = /<[A-Z][\w-]*(?:\s|\/|>)/u.test(body);
        if (!hasVisualIcon) {
          const line = source.slice(0, match.index).split(/\r?\n/u).length;
          violations.push(`${file}:${line}`);
        }
      }
    }

    assert.deepEqual(violations, []);
  });

  it("ships the DDNS copy action label in every supported language", () => {
    for (const locale of ["zh-CN", "zh-Hant", "en", "ja-JP", "ko-KR"]) {
      const source = readSource(
        `../../../packages/i18n/src/messages/admin/${locale}.ts`,
      );
      assert.match(source, /copyAddressAria:/u, locale);
    }
  });

  it("keeps automated color contrast out of the runtime audit", () => {
    const source = readSource("../../../scripts/a11y-runtime-audit.mjs");
    assert.match(
      source,
      /\.disableRules\(\["color-contrast"\]\)/u,
    );
  });
});
