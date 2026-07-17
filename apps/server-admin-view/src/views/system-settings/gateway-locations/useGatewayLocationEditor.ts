import { computed, reactive, ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import { toast } from "@admin-shared/utils/toast";
import type { HostLocation, HostLocationAction } from "@/types";
import {
  cleanHostLocationPath,
  cloneLocation,
  createDefaultLocation,
  createDefaultLocationForm,
  DEFAULT_RESPONSE_CONTENT_TYPE,
  type GatewayLocationForm,
  type GatewayLocationHeaderRow,
} from "./gatewayLocationModel";

const forbiddenResponseHeaders = new Set([
  "connection",
  "keep-alive",
  "proxy-connection",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
  "content-length",
  "content-type",
]);

const headersToRows = (
  headers: Record<string, string>,
): GatewayLocationHeaderRow[] =>
  Object.entries(headers).map(([name, value]) => ({ name, value }));

const rowsToHeaders = (
  rows: GatewayLocationHeaderRow[],
): Record<string, string> => {
  const headers: Record<string, string> = {};
  for (const row of rows) {
    const name = row.name.trim();
    if (name) headers[name] = row.value;
  }
  return headers;
};

const isValidHeaderName = (value: string): boolean =>
  /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(value);

export const useGatewayLocationEditor = (options: {
  draftLocations: Ref<HostLocation[]>;
  persistLocations: (locations: HostLocation[]) => Promise<boolean>;
}) => {
  const { t } = useI18n();
  const editingIndex = ref<number | null>(null);
  const isDialogOpen = ref(false);
  const form = reactive<GatewayLocationForm>(createDefaultLocationForm());

  const isProxyLocationWebSocketTarget = computed(
    () => form.action === "proxy" && isWebSocketProxyTargetUrl(form.target),
  );

  const formError = computed(() => {
    const rawLocationPath = form.path.trim();
    if (!rawLocationPath) {
      return t("admin.gatewayLocationsSettings.pathRequired");
    }
    if (!rawLocationPath.startsWith("/")) {
      return t("admin.gatewayLocationsSettings.pathMustStartSlash");
    }

    const locationPath = cleanHostLocationPath(rawLocationPath);
    if (locationPath === "/") {
      return t("admin.gatewayLocationsSettings.rootPathForbidden");
    }
    if (
      locationPath.startsWith("/__") ||
      locationPath === "/s" ||
      locationPath === "/s/"
    ) {
      return t("admin.gatewayLocationsSettings.reservedPathForbidden");
    }

    const duplicate = options.draftLocations.value.some(
      (location, index) =>
        index !== editingIndex.value &&
        location.path === locationPath &&
        location.match === form.match,
    );
    if (duplicate) {
      return t("admin.gatewayLocationsSettings.duplicatePath");
    }
    if (form.action === "proxy" && !form.target.trim()) {
      return t("admin.gatewayLocationsSettings.proxyTargetRequired");
    }
    if (form.action !== "response") return "";

    const status = Math.floor(Number(form.response.status) || 0);
    if (status < 100 || status > 599) {
      return t("admin.gatewayLocationsSettings.statusRange");
    }

    const seen = new Set<string>();
    for (const row of form.headers) {
      const name = row.name.trim();
      if (!name && !row.value) continue;
      if (!name) {
        return t("admin.gatewayLocationsSettings.headerNameRequired");
      }
      if (!isValidHeaderName(name)) {
        return t("admin.gatewayLocationsSettings.invalidHeaderName", { name });
      }

      const key = name.toLowerCase();
      if (forbiddenResponseHeaders.has(key)) {
        return t("admin.gatewayLocationsSettings.forbiddenHeader", { name });
      }
      if (seen.has(key)) {
        return t("admin.gatewayLocationsSettings.duplicateHeader", { name });
      }
      seen.add(key);
    }
    return "";
  });

  const resetForm = () => {
    Object.assign(form, createDefaultLocationForm());
  };

  const openCreateDialog = () => {
    editingIndex.value = null;
    resetForm();
    isDialogOpen.value = true;
  };

  const openEditDialog = (index: number) => {
    const location = options.draftLocations.value[index];
    if (!location) return;
    editingIndex.value = index;
    Object.assign(form, cloneLocation(location), {
      headers: headersToRows(location.response?.headers ?? {}),
    });
    isDialogOpen.value = true;
  };

  const closeDialog = () => {
    isDialogOpen.value = false;
    editingIndex.value = null;
    resetForm();
  };

  const setAction = (action: HostLocationAction) => {
    form.action = action;
    const usesProxy = action === "proxy";
    form.strip_path = usesProxy;
    form.rewrite_html = usesProxy;
  };

  const addHeaderRow = () => {
    form.headers.push({ name: "", value: "" });
  };

  const removeHeaderRow = (index: number) => {
    form.headers.splice(index, 1);
  };

  const buildLocationFromForm = (): HostLocation => {
    const action = form.action;
    const isWebSocketProxy =
      action === "proxy" && isWebSocketProxyTargetUrl(form.target);
    return {
      path: cleanHostLocationPath(form.path),
      match: form.match,
      action,
      target: action === "proxy" ? form.target.trim() : "",
      strip_path: action === "proxy" ? form.strip_path : false,
      rewrite_html:
        action === "proxy" && !isWebSocketProxy ? form.rewrite_html : false,
      response:
        action === "response"
          ? {
              status: Math.floor(Number(form.response.status) || 200),
              content_type:
                form.response.content_type.trim() ||
                DEFAULT_RESPONSE_CONTENT_TYPE,
              headers: rowsToHeaders(form.headers),
              body: form.response.body,
            }
          : createDefaultLocation().response,
    };
  };

  const saveDialogLocation = async () => {
    if (formError.value) {
      toast.error(t("admin.gatewayLocationsSettings.ruleNotSaved"), {
        description: formError.value,
      });
      return;
    }

    const nextLocation = buildLocationFromForm();
    const nextLocations =
      editingIndex.value === null
        ? [...options.draftLocations.value, nextLocation]
        : options.draftLocations.value.map((location, index) =>
            index === editingIndex.value ? nextLocation : location,
          );

    if (await options.persistLocations(nextLocations)) closeDialog();
  };

  const removeLocation = (index: number) => {
    options.draftLocations.value = options.draftLocations.value.filter(
      (_, itemIndex) => itemIndex !== index,
    );
  };

  return {
    addHeaderRow,
    closeDialog,
    editingIndex,
    form,
    formError,
    isDialogOpen,
    isProxyLocationWebSocketTarget,
    openCreateDialog,
    openEditDialog,
    removeHeaderRow,
    removeLocation,
    saveDialogLocation,
    setAction,
  };
};
