import { computed, effectScope, nextTick, ref } from "vue";
import { flushPromises } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ConfigAPI,
  type StaticPathBrowseEntry,
  type StaticPathBrowseResult,
  type StaticPathProbeResult,
  type StaticPathProbeTargetType,
} from "../src/lib/api/config";
import { useStaticPathBrowser } from "../src/views/subdomain-proxy/useStaticPathBrowser";

const directoryEntry = (
  path: string,
  name = path.split("/").at(-1) || path,
): StaticPathBrowseEntry => ({
  entry_type: "directory",
  modified_at: "2026-08-30T13:48:11Z",
  name,
  navigable: true,
  path,
  selectable: false,
  size_bytes: null,
});

const fileEntry = (path: string): StaticPathBrowseEntry => ({
  entry_type: "file",
  modified_at: "2026-08-30T13:48:11Z",
  name: path.split("/").at(-1) || path,
  navigable: false,
  path,
  selectable: true,
  size_bytes: 4096,
});

const browseResult = (
  targetType: StaticPathProbeTargetType,
  overrides: Partial<StaticPathBrowseResult> = {},
): StaticPathBrowseResult => ({
  breadcrumbs: [{ name: "srv", path: "/srv" }],
  current_path: "/srv",
  current_selectable: targetType === "directory",
  entries: [],
  error_code: null,
  next_cursor: null,
  parent_path: "/",
  platform: "posix",
  previous_cursor: null,
  selected_path: null,
  target_type: targetType,
  ...overrides,
});

const successfulProbe = (
  targetType: StaticPathProbeTargetType,
  path: string,
): StaticPathProbeResult => ({
  actual_type: targetType,
  error_code: null,
  exists: true,
  normalized_path: path,
  readable: true,
  target_type: targetType,
});

const createHarness = (
  initialTargetType: StaticPathProbeTargetType = "directory",
) => {
  const scope = effectScope();
  const targetType = ref<StaticPathProbeTargetType | null>(initialTargetType);
  const isDialogOpen = ref(true);
  const view = ref<"basic" | "path-browser">("basic");
  const appliedPaths: string[] = [];
  const browser = scope.run(() =>
    useStaticPathBrowser({
      active: computed(() => view.value === "path-browser"),
      applyPath: (path) => appliedPaths.push(path),
      currentTargetType: computed(() => targetType.value),
      isDialogOpen,
      openView: () => {
        view.value = "path-browser";
      },
      returnBasicView: () => {
        view.value = "basic";
      },
      translate: (key) => key,
    }),
  );
  if (!browser) throw new Error("path browser harness failed");
  return { appliedPaths, browser, isDialogOpen, scope, targetType, view };
};

afterEach(() => {
  vi.restoreAllMocks();
});

describe("static path browser", () => {
  it("selects the current directory only after a successful path probe", async () => {
    const browse = vi
      .spyOn(ConfigAPI, "browseHostMappingStaticPath")
      .mockResolvedValue(
        browseResult("directory", {
          current_path: "/srv/docs",
          current_selectable: true,
        }),
      );
    const probe = vi
      .spyOn(ConfigAPI, "probeHostMappingStaticPath")
      .mockResolvedValue(successfulProbe("directory", "/srv/docs"));
    const harness = createHarness();

    harness.browser.openPathBrowser("directory", "/srv/docs");
    await flushPromises();

    expect(browse).toHaveBeenCalledWith("directory", "/srv/docs", null);
    expect(harness.browser.selectionPath.value).toBe("/srv/docs");
    expect(harness.appliedPaths).toEqual([]);

    await harness.browser.confirmSelection();

    expect(probe).toHaveBeenCalledWith("directory", "/srv/docs");
    expect(harness.appliedPaths).toEqual(["/srv/docs"]);
    expect(harness.view.value).toBe("basic");
    harness.scope.stop();
  });

  it("selects files, navigates folders, and binds cursor requests to the current path", async () => {
    const manual = fileEntry("/srv/manual.pdf");
    const docs = directoryEntry("/srv/docs", "docs");
    const browse = vi
      .spyOn(ConfigAPI, "browseHostMappingStaticPath")
      .mockResolvedValueOnce(
        browseResult("file", {
          entries: [docs, manual],
          next_cursor: "next-page",
        }),
      )
      .mockResolvedValueOnce(
        browseResult("file", {
          current_path: "/srv",
          previous_cursor: "previous-page",
        }),
      )
      .mockResolvedValueOnce(
        browseResult("file", { current_path: "/srv/docs" }),
      );
    const harness = createHarness("file");

    harness.browser.openPathBrowser("file", "");
    await flushPromises();
    harness.browser.activateEntry(manual);
    expect(harness.browser.selectedPath.value).toBe("/srv/manual.pdf");

    harness.browser.loadNextPage();
    await flushPromises();
    expect(browse).toHaveBeenNthCalledWith(2, "file", "/srv", "next-page");
    expect(harness.browser.selectedPath.value).toBe("/srv/manual.pdf");

    harness.browser.activateEntry(docs);
    await flushPromises();
    expect(browse).toHaveBeenNthCalledWith(3, "file", "/srv/docs", null);
    expect(harness.browser.selectedPath.value).toBeNull();
    harness.scope.stop();
  });

  it("ignores stale navigation responses and cancels when the target type changes", async () => {
    let resolveInitial!: (value: StaticPathBrowseResult) => void;
    const initial = new Promise<StaticPathBrowseResult>((resolve) => {
      resolveInitial = resolve;
    });
    vi.spyOn(ConfigAPI, "browseHostMappingStaticPath")
      .mockReturnValueOnce(initial)
      .mockResolvedValueOnce(
        browseResult("directory", { current_path: "/newer" }),
      );
    const harness = createHarness();

    harness.browser.openPathBrowser("directory", "/older");
    await harness.browser.navigateRoot();
    expect(harness.browser.currentPath.value).toBe("/newer");

    resolveInitial(
      browseResult("directory", { current_path: "/stale-response" }),
    );
    await flushPromises();
    expect(harness.browser.currentPath.value).toBe("/newer");

    harness.targetType.value = "file";
    await nextTick();
    expect(harness.view.value).toBe("basic");
    expect(harness.browser.result.value).toBeNull();
    harness.scope.stop();
  });

  it("treats an empty Windows drive parent as the virtual root", async () => {
    const browse = vi
      .spyOn(ConfigAPI, "browseHostMappingStaticPath")
      .mockResolvedValueOnce(
        browseResult("directory", {
          current_path: "C:\\",
          parent_path: "",
          platform: "windows",
        }),
      )
      .mockResolvedValueOnce(
        browseResult("directory", {
          breadcrumbs: [],
          current_path: null,
          current_selectable: false,
          parent_path: null,
          platform: "windows",
        }),
      );
    const harness = createHarness();

    harness.browser.openPathBrowser("directory", "C:\\");
    await flushPromises();
    expect(harness.browser.parentPath.value).toBe("");

    harness.browser.navigateParent();
    await flushPromises();
    expect(browse).toHaveBeenNthCalledWith(2, "directory", "", null);
    expect(harness.browser.currentPath.value).toBeNull();
    harness.scope.stop();
  });

  it("drops an in-flight response after the mapping dialog closes", async () => {
    let resolveBrowse!: (value: StaticPathBrowseResult) => void;
    vi.spyOn(ConfigAPI, "browseHostMappingStaticPath").mockReturnValue(
      new Promise<StaticPathBrowseResult>((resolve) => {
        resolveBrowse = resolve;
      }),
    );
    const harness = createHarness();

    harness.browser.openPathBrowser("directory", "/srv/slow");
    harness.isDialogOpen.value = false;
    await nextTick();
    resolveBrowse(
      browseResult("directory", { current_path: "/srv/stale-after-close" }),
    );
    await flushPromises();

    expect(harness.browser.result.value).toBeNull();
    expect(harness.browser.isLoading.value).toBe(false);
    harness.scope.stop();
  });

  it("surfaces stable browse errors without making a path selectable", async () => {
    vi.spyOn(ConfigAPI, "browseHostMappingStaticPath").mockResolvedValue(
      browseResult("directory", {
        current_path: "/oversized",
        current_selectable: false,
        error_code: "directory_too_large",
      }),
    );
    const harness = createHarness();

    harness.browser.openPathBrowser("directory", "/oversized");
    await flushPromises();

    expect(harness.browser.canConfirm.value).toBe(false);
    expect(harness.browser.loadError.value).toBe(
      "admin.subdomainProxy.staticServe.browser.errors.directory_too_large",
    );
    harness.scope.stop();
  });

  it("keeps the mapping draft unchanged when the confirmation probe fails", async () => {
    vi.spyOn(ConfigAPI, "browseHostMappingStaticPath").mockResolvedValue(
      browseResult("file", {
        entries: [fileEntry("/srv/manual.pdf")],
      }),
    );
    vi.spyOn(ConfigAPI, "probeHostMappingStaticPath").mockResolvedValue({
      actual_type: null,
      error_code: "not_found",
      exists: false,
      normalized_path: "",
      readable: false,
      target_type: "file",
    });
    const harness = createHarness("file");

    harness.browser.openPathBrowser("file", "");
    await flushPromises();
    harness.browser.activateEntry(fileEntry("/srv/manual.pdf"));
    await harness.browser.confirmSelection();

    expect(harness.appliedPaths).toEqual([]);
    expect(harness.view.value).toBe("path-browser");
    expect(harness.browser.confirmError.value).toBe(
      "admin.subdomainProxy.staticServe.probeErrors.not_found",
    );
    harness.scope.stop();
  });

  it("preserves POSIX trailing spaces and rejects a probe replacement path", async () => {
    const selected = "/srv/public ";
    const adjacent = "/srv/public";
    const browse = vi
      .spyOn(ConfigAPI, "browseHostMappingStaticPath")
      .mockResolvedValue(
        browseResult("directory", {
          current_path: selected,
          current_selectable: true,
        }),
      );
    const probe = vi
      .spyOn(ConfigAPI, "probeHostMappingStaticPath")
      .mockResolvedValue(successfulProbe("directory", adjacent));
    const harness = createHarness();

    harness.browser.openPathBrowser("directory", selected);
    await flushPromises();

    expect(browse).toHaveBeenCalledWith("directory", selected, null);
    expect(harness.browser.selectionPath.value).toBe(selected);

    await harness.browser.confirmSelection();

    expect(probe).toHaveBeenCalledWith("directory", selected);
    expect(harness.appliedPaths).toEqual([]);
    expect(harness.view.value).toBe("path-browser");
    expect(harness.browser.confirmError.value).toBe(
      "admin.subdomainProxy.staticServe.browser.errors.invalid_response",
    );
    harness.scope.stop();
  });

  it("ignores a confirmation probe after navigating to another directory", async () => {
    const manual = fileEntry("/srv/manual.pdf");
    const docs = directoryEntry("/srv/docs", "docs");
    vi.spyOn(ConfigAPI, "browseHostMappingStaticPath")
      .mockResolvedValueOnce(
        browseResult("file", {
          entries: [docs, manual],
        }),
      )
      .mockResolvedValueOnce(
        browseResult("file", { current_path: "/srv/docs" }),
      );
    let resolveProbe!: (value: StaticPathProbeResult) => void;
    vi.spyOn(ConfigAPI, "probeHostMappingStaticPath").mockReturnValue(
      new Promise<StaticPathProbeResult>((resolve) => {
        resolveProbe = resolve;
      }),
    );
    const harness = createHarness("file");

    harness.browser.openPathBrowser("file", "");
    await flushPromises();
    harness.browser.activateEntry(manual);
    const confirmation = harness.browser.confirmSelection();
    expect(harness.browser.isConfirming.value).toBe(true);

    harness.browser.activateEntry(docs);
    await flushPromises();
    expect(harness.browser.currentPath.value).toBe("/srv/docs");
    expect(harness.browser.isConfirming.value).toBe(false);

    resolveProbe(successfulProbe("file", "/srv/manual.pdf"));
    await confirmation;

    expect(harness.appliedPaths).toEqual([]);
    expect(harness.view.value).toBe("path-browser");
    expect(harness.browser.currentPath.value).toBe("/srv/docs");
    harness.scope.stop();
  });
});
