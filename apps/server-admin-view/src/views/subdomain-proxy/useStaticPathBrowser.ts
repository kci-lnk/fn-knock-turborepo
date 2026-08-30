import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import {
  ConfigAPI,
  type StaticPathBrowseEntry,
  type StaticPathBrowseResult,
  type StaticPathProbeTargetType,
} from "@/lib/api/config";
import type { TranslationParams } from "./model";

type Translate = (key: string, params?: TranslationParams) => string;

const isSuccessfulProbe = (
  result: Awaited<ReturnType<typeof ConfigAPI.probeHostMappingStaticPath>>,
  targetType: StaticPathProbeTargetType,
) =>
  !result.error_code &&
  result.exists &&
  result.readable &&
  result.target_type === targetType &&
  result.actual_type === targetType;

export const useStaticPathBrowser = ({
  active,
  applyPath,
  currentTargetType,
  isDialogOpen,
  openView,
  returnBasicView,
  translate,
}: {
  active: ComputedRef<boolean>;
  applyPath: (path: string) => void;
  currentTargetType: ComputedRef<StaticPathProbeTargetType | null>;
  isDialogOpen: Ref<boolean>;
  openView: () => void;
  returnBasicView: () => void;
  translate: Translate;
}) => {
  const targetType = ref<StaticPathProbeTargetType>("directory");
  const result = ref<StaticPathBrowseResult | null>(null);
  const selectedPath = ref<string | null>(null);
  const isLoading = ref(false);
  const isConfirming = ref(false);
  const loadError = ref("");
  const confirmError = ref("");
  const requestedPath = ref<string | null>(null);
  const pathDraft = ref("");
  let browseRequestId = 0;
  let confirmRequestId = 0;

  const currentPath = computed(() => result.value?.current_path ?? null);
  const parentPath = computed(() => result.value?.parent_path ?? null);
  const breadcrumbs = computed(() => result.value?.breadcrumbs ?? []);
  const entries = computed(() => result.value?.entries ?? []);
  const previousCursor = computed(() => result.value?.previous_cursor ?? null);
  const nextCursor = computed(() => result.value?.next_cursor ?? null);
  const pathDraftMatchesCurrent = computed(
    () => pathDraft.value === (currentPath.value ?? ""),
  );
  const selectionPath = computed(() => {
    if (!pathDraftMatchesCurrent.value) return null;
    if (targetType.value === "file") return selectedPath.value;
    return result.value?.current_selectable
      ? (result.value.current_path ?? null)
      : null;
  });
  const canConfirm = computed(
    () =>
      Boolean(selectionPath.value) &&
      !isLoading.value &&
      !isConfirming.value &&
      !loadError.value,
  );

  const invalidateConfirmation = () => {
    confirmRequestId += 1;
    isConfirming.value = false;
  };

  const invalidateBrowse = () => {
    browseRequestId += 1;
    isLoading.value = false;
  };

  const invalidateRequests = () => {
    invalidateBrowse();
    invalidateConfirmation();
  };

  const clearState = () => {
    invalidateRequests();
    result.value = null;
    selectedPath.value = null;
    requestedPath.value = null;
    pathDraft.value = "";
    loadError.value = "";
    confirmError.value = "";
  };

  const browse = async (
    path: string | null,
    cursor: string | null = null,
    preserveSelection = false,
  ) => {
    // A navigation changes the selection context. Invalidate an in-flight
    // confirmation probe so its older path can never be applied after the
    // user has moved to another directory or page.
    invalidateConfirmation();
    const requestId = ++browseRequestId;
    const requestedTargetType = targetType.value;
    const requestedDraft = path ?? "";
    requestedPath.value = path;
    pathDraft.value = requestedDraft;
    isLoading.value = true;
    loadError.value = "";
    confirmError.value = "";
    try {
      const payload = await ConfigAPI.browseHostMappingStaticPath(
        requestedTargetType,
        path,
        cursor,
      );
      if (
        requestId !== browseRequestId ||
        !active.value ||
        currentTargetType.value !== requestedTargetType
      ) {
        return;
      }
      if (payload.target_type !== requestedTargetType) {
        result.value = null;
        selectedPath.value = null;
        loadError.value = translate(
          "admin.subdomainProxy.staticServe.browser.errors.invalid_response",
        );
        return;
      }
      result.value = payload;
      if (!preserveSelection) selectedPath.value = payload.selected_path;
      if (payload.error_code) {
        loadError.value = translate(
          `admin.subdomainProxy.staticServe.browser.errors.${payload.error_code}`,
        );
        return;
      }
      // Keep edits made while the request was in flight. Otherwise, reflect
      // the server-authoritative directory (a file request resolves to its
      // parent while selected_path keeps the file selection).
      if (pathDraft.value === requestedDraft) {
        pathDraft.value = payload.current_path ?? "";
      }
    } catch (error) {
      if (requestId !== browseRequestId || !active.value) return;
      result.value = null;
      selectedPath.value = null;
      loadError.value = extractErrorMessage(
        error,
        translate(
          "admin.subdomainProxy.staticServe.browser.errors.browse_failed",
        ),
      );
    } finally {
      if (requestId === browseRequestId) isLoading.value = false;
    }
  };

  const openPathBrowser = (
    requestedTargetType: StaticPathProbeTargetType,
    initialPath: string,
  ) => {
    if (
      !isDialogOpen.value ||
      currentTargetType.value !== requestedTargetType
    ) {
      return;
    }
    clearState();
    targetType.value = requestedTargetType;
    openView();
    void browse(initialPath === "" ? null : initialPath);
  };

  const cancel = () => {
    clearState();
    returnBasicView();
  };

  const navigateRoot = () => browse(null);
  const navigateToPath = () =>
    browse(pathDraft.value === "" ? null : pathDraft.value);
  const updatePathDraft = (value: string | number) => {
    const nextPath = String(value);
    if (nextPath === pathDraft.value) return;
    pathDraft.value = nextPath;
    invalidateBrowse();
    invalidateConfirmation();
    loadError.value = "";
    confirmError.value = "";
  };
  const navigateParent = () => {
    if (parentPath.value !== null) void browse(parentPath.value);
  };
  const navigateBreadcrumb = (path: string) => void browse(path);
  const refresh = () => void browse(currentPath.value ?? requestedPath.value);
  const loadPreviousPage = () => {
    if (previousCursor.value) {
      void browse(currentPath.value, previousCursor.value, true);
    }
  };
  const loadNextPage = () => {
    if (nextCursor.value) {
      void browse(currentPath.value, nextCursor.value, true);
    }
  };

  const activateEntry = (entry: StaticPathBrowseEntry) => {
    confirmError.value = "";
    if (entry.entry_type === "directory" && entry.navigable) {
      void browse(entry.path);
      return;
    }
    if (
      targetType.value === "file" &&
      entry.entry_type === "file" &&
      entry.selectable
    ) {
      invalidateConfirmation();
      pathDraft.value = currentPath.value ?? "";
      selectedPath.value = entry.path;
    }
  };

  const confirmSelection = async () => {
    const path = selectionPath.value;
    if (!path || !canConfirm.value) return;
    const requestId = ++confirmRequestId;
    const requestedTargetType = targetType.value;
    isConfirming.value = true;
    confirmError.value = "";
    try {
      const probe = await ConfigAPI.probeHostMappingStaticPath(
        requestedTargetType,
        path,
      );
      if (
        requestId !== confirmRequestId ||
        !active.value ||
        currentTargetType.value !== requestedTargetType
      ) {
        return;
      }
      if (!isSuccessfulProbe(probe, requestedTargetType)) {
        const code =
          probe.error_code ||
          (probe.actual_type !== requestedTargetType
            ? "type_mismatch"
            : "probe_failed");
        confirmError.value = translate(
          `admin.subdomainProxy.staticServe.probeErrors.${code}`,
        );
        return;
      }
      if (probe.normalized_path !== path) {
        confirmError.value = translate(
          "admin.subdomainProxy.staticServe.browser.errors.invalid_response",
        );
        return;
      }
      applyPath(path);
      clearState();
      returnBasicView();
    } catch (error) {
      if (requestId !== confirmRequestId || !active.value) return;
      confirmError.value = extractErrorMessage(
        error,
        translate("admin.subdomainProxy.staticServe.probeErrors.probe_failed"),
      );
    } finally {
      if (requestId === confirmRequestId) isConfirming.value = false;
    }
  };

  watch(currentTargetType, (nextTargetType) => {
    if (active.value && nextTargetType !== targetType.value) cancel();
  });
  watch(isDialogOpen, (open) => {
    if (!open) clearState();
  });

  return {
    activateEntry,
    breadcrumbs,
    canConfirm,
    cancel,
    confirmError,
    confirmSelection,
    currentPath,
    entries,
    isConfirming,
    isLoading,
    loadError,
    loadNextPage,
    loadPreviousPage,
    navigateBreadcrumb,
    navigateParent,
    navigateRoot,
    navigateToPath,
    nextCursor,
    openPathBrowser,
    pathDraft,
    parentPath,
    previousCursor,
    refresh,
    result,
    selectedPath,
    selectionPath,
    targetType,
    updatePathDraft,
  };
};
