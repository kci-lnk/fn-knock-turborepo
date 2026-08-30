import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { computed, ref } from "vue";

import { useStaleHostMappingsCleanup } from "../src/composables/useStaleHostMappingsCleanup";
import { toHostMappingUpdatePayload } from "../src/lib/api/host-mapping-payload";
import {
  createDefaultStaticServe,
  getStaticServeValidationIssue,
  normalizeHostMappingStaticServe,
  normalizeHostMappingTargetType,
} from "../src/lib/host-mapping-target";
import { hasPendingHostMappingMetadata } from "../src/store/hostMappingMetadata";
import {
  createDefaultMapping,
  normalizeMappingForm,
} from "../src/views/subdomain-proxy/model";
import { buildTargetOptimizationPreviews } from "../src/views/subdomain-proxy/subdomain-target-optimization";
import { useSubdomainMappingsView } from "../src/views/subdomain-proxy/useSubdomainMappingsView";

const staticDirectoryMapping = () => ({
  ...createDefaultMapping(),
  host: "docs.example.test",
  target_type: "directory" as const,
  target: "http://stale-upstream:8080",
  static_serve: {
    path: "/srv/docs",
    index_files: ["index.html", "index.htm"],
    directory_listing: { enabled: true, render_readme: true },
  },
});

describe("host mapping static target model", () => {
  it("keeps missing and unknown target types backward-compatible with proxy", () => {
    assert.equal(normalizeHostMappingTargetType(undefined), "proxy");
    assert.equal(normalizeHostMappingTargetType("legacy"), "proxy");
    assert.equal(normalizeHostMappingTargetType("file"), "file");
  });

  it("uses stable defaults and normalizes file and directory shapes", () => {
    assert.deepEqual(createDefaultStaticServe("directory"), {
      path: "",
      index_files: ["index.html", "index.htm"],
      directory_listing: { enabled: false, render_readme: false },
    });
    assert.deepEqual(
      normalizeHostMappingStaticServe("file", {
        path: "/srv/manual.pdf ",
        index_files: ["index.html"],
        directory_listing: { enabled: true, render_readme: true },
      }),
      {
        path: "/srv/manual.pdf ",
        index_files: [],
        directory_listing: { enabled: false, render_readme: false },
      },
    );
    assert.equal(
      normalizeHostMappingStaticServe("file", {
        path: "   ",
        index_files: [],
        directory_listing: { enabled: false, render_readme: false },
      })?.path,
      "",
    );
    assert.equal(
      normalizeHostMappingStaticServe("directory", {
        path: "/srv/docs",
        index_files: [],
        directory_listing: { enabled: false, render_readme: true },
      })?.directory_listing.render_readme,
      false,
    );
  });

  it("validates raw default-document drafts without hiding excess or duplicates", () => {
    const directory = createDefaultStaticServe("directory");
    directory.path = "/srv/docs";
    directory.index_files = Array.from(
      { length: 17 },
      (_, index) => `index-${index}.html`,
    );
    assert.equal(
      getStaticServeValidationIssue({
        staticServe: directory,
        targetType: "directory",
      }),
      "too_many_index_files",
    );

    directory.index_files = ["index.html", " index.html "];
    assert.equal(
      getStaticServeValidationIssue({
        staticServe: directory,
        targetType: "directory",
      }),
      "duplicate_index_file",
    );

    directory.index_files = [`${"界".repeat(85)}a`];
    assert.equal(
      getStaticServeValidationIssue({
        staticServe: directory,
        targetType: "directory",
      }),
      "invalid_index_file",
    );
  });

  it("validates absolute paths using the active server platform", () => {
    const directory = createDefaultStaticServe("directory");
    directory.path = "C:\\Sites\\docs";
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: true,
        staticServe: directory,
        targetType: "directory",
      }),
      null,
    );
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: false,
        staticServe: directory,
        targetType: "directory",
      }),
      "path_not_absolute",
    );
    directory.path = "/srv/../private";
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: false,
        staticServe: directory,
        targetType: "directory",
      }),
      "path_has_parent_segment",
    );
    directory.path = "/srv/docs ";
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: false,
        staticServe: directory,
        targetType: "directory",
      }),
      null,
    );
    directory.path = "C:\\Sites\\docs ";
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: true,
        staticServe: directory,
        targetType: "directory",
      }),
      "path_unsafe",
    );
  });

  it("rejects UNC-like prefixes before probing on every platform hint", () => {
    const directory = createDefaultStaticServe("directory");
    directory.path = "//server/share/docs";
    for (const isWindows of [undefined, false, true]) {
      assert.equal(
        getStaticServeValidationIssue({
          isWindows,
          staticServe: directory,
          targetType: "directory",
        }),
        "path_unsafe",
      );
    }
  });

  it("rejects hidden targets, filesystem roots and control characters before probing", () => {
    const directory = createDefaultStaticServe("directory");
    for (const path of [
      "/",
      "//server/share/docs",
      "/\\server/share/docs",
      "/srv/.secret",
      "/srv/line\nbreak",
      "/srv/bidirectional-\u202etxt",
      "/srv/back\\slash",
    ]) {
      directory.path = path;
      assert.equal(
        getStaticServeValidationIssue({
          isWindows: false,
          staticServe: directory,
          targetType: "directory",
        }),
        "path_unsafe",
      );
    }

    directory.path = "C:\\";
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: true,
        staticServe: directory,
        targetType: "directory",
      }),
      "path_unsafe",
    );
    directory.path = "\\\\server\\share";
    assert.equal(
      getStaticServeValidationIssue({
        isWindows: true,
        staticServe: directory,
        targetType: "directory",
      }),
      "path_unsafe",
    );
    for (const path of [
      "C:\\Sites\\CON",
      "C:\\Sites\\report.txt:secret",
      "C:\\Sites\\report?.txt",
      "C:\\Sites\\report*.txt",
      "C:\\Sites\\report<draft>.txt",
      "C:\\Sites\\report|draft.txt",
      'C:\\Sites\\report"draft.txt',
      "C:\\Sites\\report.txt.",
      "\\\\.\\C:\\Sites\\docs",
      "\\\\?\\C:\\Sites\\docs",
      "\\\\localhost\\C$\\Windows\\System32",
      "\\\\127.0.0.1\\C$\\Windows",
      "\\\\localhost\\pipe\\fn-knock-test",
      "\\\\localhost\\mailslot\\fn-knock-test",
      "//server/share/docs",
      "\\/server/share/docs",
      "\\\\?\\UNC\\server\\share\\docs",
    ]) {
      directory.path = path;
      assert.equal(
        getStaticServeValidationIssue({
          isWindows: true,
          staticServe: directory,
          targetType: "directory",
        }),
        "path_unsafe",
      );
    }
  });

  it("rejects hidden and bidi-formatted default documents", () => {
    const directory = createDefaultStaticServe("directory");
    directory.path = "/srv/docs";
    for (const filename of [
      ".index.html",
      "line\nbreak.html",
      "safe-\u202elmth",
    ]) {
      directory.index_files = [filename];
      assert.equal(
        getStaticServeValidationIssue({
          staticServe: directory,
          targetType: "directory",
        }),
        "invalid_index_file",
      );
    }
    directory.path = "C:\\Sites\\docs";
    for (const filename of [
      "CON",
      "com1.html",
      "report.html:secret",
      "report?.html",
      "report*.html",
      "report<draft>.html",
      "report|draft.html",
      'report"draft.html',
      "report.html.",
    ]) {
      directory.index_files = [filename];
      assert.equal(
        getStaticServeValidationIssue({
          isWindows: true,
          staticServe: directory,
          targetType: "directory",
        }),
        "invalid_index_file",
      );
    }
  });

  it("canonicalizes static saves and preserves inbound policy settings", () => {
    const mapping = staticDirectoryMapping();
    mapping.static_serve.path = "/srv/docs ";
    mapping.target_path_mode = "prefix";
    mapping.preserve_host = true;
    mapping.suppress_toolbar = false;
    mapping.protocol_mode = "http2";
    mapping.waf_enabled = false;
    mapping.use_auth = true;
    mapping.basic_auth = {
      enabled: true,
      username: "upstream",
      password: "secret",
    };
    mapping.locations = [
      {
        path: "/api",
        match: "prefix",
        action: "proxy",
        target: "http://api:8080",
        strip_path: false,
        rewrite_html: false,
        auth_mode: "inherit",
        response: {
          status: 200,
          content_type: "text/plain",
          headers: {},
          body: "",
        },
      },
    ];

    const normalized = normalizeMappingForm(mapping, {
      hasFreshFaviconMetadata: true,
      hasFreshTitleMetadata: true,
      host: mapping.host,
      isAuthServiceTarget: () => false,
      isWebSocketTarget: () => false,
    });
    const payload = toHostMappingUpdatePayload(normalized);

    assert.equal(payload.target_type, "directory");
    assert.equal(payload.target, "");
    assert.deepEqual(payload.static_serve, mapping.static_serve);
    assert.equal(payload.target_path_mode, "entry");
    assert.equal(payload.preserve_host, false);
    assert.equal(payload.suppress_toolbar, true);
    assert.deepEqual(payload.basic_auth, {
      enabled: false,
      username: "",
      password: "",
    });
    assert.deepEqual(payload.locations, []);
    assert.equal(payload.protocol_mode, "http2");
    assert.equal(payload.waf_enabled, false);
    assert.equal(payload.use_auth, true);
  });

  it("excludes static targets from upstream maintenance and metadata work", () => {
    const mapping = staticDirectoryMapping();
    assert.equal(hasPendingHostMappingMetadata([mapping]), false);
    assert.deepEqual(
      buildTargetOptimizationPreviews({
        candidates: [
          {
            address: "127.0.0.1",
            cidr: "127.0.0.1/32",
            includedInAutomaticScan: true,
            recommended: true,
            source: "loopback",
          },
          {
            address: "192.168.1.8",
            cidr: "192.168.1.8/32",
            includedInAutomaticScan: true,
            recommended: false,
            source: "interface",
          },
        ],
        destinationAddress: "192.168.1.8",
        isAuthServiceTarget: () => false,
        isDockerDeployment: false,
        mappings: [mapping],
      }),
      [],
    );
    const cleanup = useStaleHostMappingsCleanup({
      mappings: computed(() => [mapping]),
      saveMappings: async () => undefined,
      isAuthServiceTarget: () => false,
    });
    assert.deepEqual(cleanup.probeableMappings.value, []);
  });

  it("finds static mappings by raw and localized response type", () => {
    const mapping = staticDirectoryMapping();
    const searchQuery = ref("directory");
    const state = useSubdomainMappingsView({
      allMappings: computed(() => [mapping]),
      draggableVisibleMappings: ref([]),
      formatHostWithAccessEntryPort: (host) => host,
      groups: computed(() => []),
      isAuthServiceTarget: () => false,
      searchQuery,
      trafficRealtimeStats: ref(null),
      translate: (key) => (key.endsWith(".directory") ? "Folder" : key),
    });
    assert.deepEqual(
      state.filteredMappings.value.map((item) => item.host),
      [mapping.host],
    );
    searchQuery.value = "folder";
    assert.deepEqual(
      state.filteredMappings.value.map((item) => item.host),
      [mapping.host],
    );
  });
});
