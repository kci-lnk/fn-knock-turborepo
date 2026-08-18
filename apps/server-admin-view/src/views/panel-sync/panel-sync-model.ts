import type {
  PanelConnection,
  PanelConnectionInput,
  PanelConnectionUpdateInput,
  PanelProvider,
} from "@/lib/api/panel-sync-api";

export const panelApiPaths: Record<PanelProvider, string> = {
  sun_panel: "/openapi/v1",
  one_nav: "/index.php?c=api",
  van_nav: "/api",
};

export const panelProviderNames: Record<PanelProvider, string> = {
  sun_panel: "Sun-Panel",
  one_nav: "OneNav",
  van_nav: "Van-Nav",
};

export type PanelSyncEditorForm = Omit<
  PanelConnectionInput,
  "api_path" | "base_url"
> & {
  endpoint_url: string;
};

const normalizePath = (value: string): string => {
  const path = value.trim();
  if (!path) return "/";
  return path.startsWith("/") ? path : `/${path}`;
};

export const composePanelEndpointUrl = (
  baseUrl: string,
  apiPath: string,
): string => {
  const base = baseUrl.trim().replace(/\/+$/u, "");
  if (!base) return "";
  return `${base}${normalizePath(apiPath)}`;
};

export const splitPanelEndpointUrl = (
  endpointUrl: string,
): { api_path: string; base_url: string } => {
  const url = new URL(endpointUrl.trim());
  if (!(["http:", "https:"] as string[]).includes(url.protocol)) {
    throw new TypeError("Only HTTP and HTTPS panel endpoints are supported");
  }
  if (url.username || url.password || url.hash) {
    throw new TypeError("Panel endpoint cannot contain credentials or a hash");
  }
  return {
    base_url: url.origin,
    api_path: `${url.pathname}${url.search}` || "/",
  };
};

export const nextPanelConnectionName = (
  provider: PanelProvider,
  existingNames: Iterable<string>,
): string => {
  const used = new Set(Array.from(existingNames, (name) => name.trim()));
  const prefix = panelProviderNames[provider];
  let index = 1;
  while (used.has(`${prefix}-${index}`)) index += 1;
  return `${prefix}-${index}`;
};

export const createPanelSyncForm = (
  existingNames: Iterable<string> = [],
): PanelSyncEditorForm => ({
  name: nextPanelConnectionName("sun_panel", existingNames),
  provider: "sun_panel",
  endpoint_url: "",
  allow_invalid_tls: false,
  grouping: {
    mode: "mirror",
    namespace: "fn-knock",
    single_group_name: "",
  },
  auto_sync: { enabled: true, interval_minutes: 60 },
  credential: "",
  clear_credential: false,
});

export const panelConnectionToForm = (
  connection: PanelConnection,
): PanelSyncEditorForm => ({
  name: connection.name,
  provider: connection.provider,
  endpoint_url: composePanelEndpointUrl(
    connection.base_url,
    connection.api_path,
  ),
  allow_invalid_tls: connection.allow_invalid_tls ?? false,
  grouping: {
    mode: connection.grouping?.mode ?? "mirror",
    namespace: connection.grouping?.namespace ?? "fn-knock",
    single_group_name: connection.grouping?.single_group_name ?? "",
  },
  auto_sync: {
    enabled: connection.auto_sync?.enabled ?? true,
    interval_minutes: connection.auto_sync?.interval_minutes ?? 60,
  },
  credential: "",
  clear_credential: false,
});

const endpointPartsForSave = (
  form: PanelSyncEditorForm,
  existing?: PanelConnection | null,
) => {
  if (
    existing &&
    composePanelEndpointUrl(existing.base_url, existing.api_path) ===
      form.endpoint_url.trim().replace(/\/+$/u, "")
  ) {
    return { base_url: existing.base_url, api_path: existing.api_path };
  }
  return splitPanelEndpointUrl(form.endpoint_url);
};

export const panelFormToUpdate = (
  form: PanelSyncEditorForm,
  existing?: PanelConnection | null,
): PanelConnectionUpdateInput => ({
  name: form.name,
  ...endpointPartsForSave(form, existing),
  allow_invalid_tls: form.allow_invalid_tls,
  grouping: {
    ...form.grouping,
    namespace: form.grouping.namespace.trim() || "fn-knock",
  },
  auto_sync: { ...form.auto_sync },
  credential: form.credential || undefined,
  clear_credential: form.clear_credential,
});

export const panelFormToInput = (
  form: PanelSyncEditorForm,
  existing?: PanelConnection | null,
): PanelConnectionInput => ({
  ...panelFormToUpdate(form, existing),
  provider: form.provider,
});

export const isPanelAutoSyncReady = (
  connection: PanelConnection | null,
  form: PanelSyncEditorForm,
): boolean =>
  connection?.verified_at != null &&
  connection.credential_configured &&
  !form.clear_credential &&
  !form.credential?.trim() &&
  composePanelEndpointUrl(connection.base_url, connection.api_path) ===
    form.endpoint_url.trim().replace(/\/+$/u, "") &&
  (connection.allow_invalid_tls ?? false) === form.allow_invalid_tls;
