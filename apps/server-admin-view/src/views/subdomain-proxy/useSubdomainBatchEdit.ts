import { computed, ref, type ComputedRef, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import {
  isSupportedProxyTargetUrl,
  isWebSocketProxyTargetUrl,
} from "@admin-shared/utils/proxyTargetInput";
import {
  batchMappingSignature as signature,
  isValidBatchMappingHost,
} from "./subdomain-batch-edit-model";
import type { HostMapping } from "@/types";
import {
  mergeGatewayDisabledHostsForMapping,
  normalizeDisabledHosts,
  getStaticServeValidationIssue,
  isProxyHostMapping,
  normalizeHostLike,
  normalizeHostMappingTargetType,
} from "./model";

export interface BatchEditRow {
  originalHost: string;
  original: HostMapping;
  title: string;
  host: string;
  target: string;
}
export type BatchGatewayKind =
  "gateway_proxy_headers" | "gateway_host_response";
interface GatewayChange {
  kind: BatchGatewayKind;
  before: string[];
  afterCatalogCleanup: string[];
  after: string[];
}
interface Options {
  allMappings: ComputedRef<HostMapping[]>;
  isSavingMappings: Ref<boolean>;
  isWindows: () => boolean;
  isAuthServiceTarget: (target: string) => boolean;
  saveHostMappings: (
    mappings: HostMapping[],
    icons?: ReadonlySet<string>,
    titles?: ReadonlySet<string>,
    previousHosts?: ReadonlyMap<string, string>,
    beforeSave?: () => void,
  ) => Promise<unknown>;
  reloadMappings: () => Promise<unknown>;
  readGatewayHosts: (kind: BatchGatewayKind) => Promise<string[]>;
  writeGatewayHosts: (kind: BatchGatewayKind, hosts: string[]) => Promise<void>;
  translate: (key: string, params?: Record<string, string | number>) => string;
  onSaved: () => void;
}
const hostList = (hosts: string[]) =>
  [...new Set(hosts.map(normalizeHostLike))].sort();
const sameHosts = (a: string[], b: string[]) =>
  JSON.stringify(hostList(a)) === JSON.stringify(hostList(b));

export const useSubdomainBatchEdit = (options: Options) => {
  const open = ref(false);
  const rows = ref<BatchEditRow[]>([]);
  const saving = ref(false);
  const mappingsSaved = ref(false);
  const discardOpen = ref(false);
  const error = ref("");
  const attempted = ref(false);
  let baseline = "";
  let initialDraft = "";
  let complete: (() => void) | undefined;
  let pendingGateway: GatewayChange[] = [];
  const t = (key: string) =>
    options.translate(`admin.subdomainProxy.batchEdit.${key}`);
  const draftSignature = () =>
    JSON.stringify(
      rows.value.map(({ title, host, target }) => ({ title, host, target })),
    );
  const dirty = computed(() => draftSignature() !== initialDraft);
  const busy = computed(() => saving.value || options.isSavingMappings.value);
  const errors = computed(() => {
    if (mappingsSaved.value)
      return rows.value.map(() => ({ host: "", target: "" }));
    const hosts = rows.value.map((row) => normalizeHostLike(row.host));
    return rows.value.map((row, index) => {
      const host = hosts[index]!;
      const target = row.target.trim();
      const original = row.original;
      const hostError = !isValidBatchMappingHost(host)
        ? t("invalidHost")
        : hosts.some((value, other) => other !== index && value === host) ||
            options.allMappings.value.some(
              (mapping) =>
                normalizeHostLike(mapping.host) === host &&
                mapping.host !== row.originalHost,
            )
          ? t("duplicateHost")
          : "";
      let targetError = "";
      if (isProxyHostMapping(original)) {
        if (!isSupportedProxyTargetUrl(target))
          targetError = t("invalidTarget");
        else if (options.isAuthServiceTarget(target))
          targetError = t("authTarget");
      } else {
        const issue = getStaticServeValidationIssue({
          isWindows: options.isWindows(),
          targetType: normalizeHostMappingTargetType(original.target_type),
          staticServe: original.static_serve
            ? { ...original.static_serve, path: target }
            : null,
        });
        if (issue) targetError = t("invalidPath");
      }
      return { host: hostError, target: targetError };
    });
  });
  const close = () => {
    open.value = false;
    discardOpen.value = false;
    rows.value = [];
    pendingGateway = [];
    complete = undefined;
  };
  const requestClose = () => {
    if (busy.value) return;
    if (dirty.value || mappingsSaved.value) discardOpen.value = true;
    else close();
  };
  const discard = () => {
    if (!busy.value) close();
  };
  const openDialog = (hosts: string[], onComplete: () => void) => {
    if (busy.value || open.value) return;
    const selected = new Set(hosts);
    rows.value = options.allMappings.value
      .filter(
        (mapping) =>
          selected.has(mapping.host) &&
          mapping.service_role !== "auth" &&
          !options.isAuthServiceTarget(mapping.target),
      )
      .map((mapping) => ({
        originalHost: mapping.host,
        original: JSON.parse(JSON.stringify(mapping)) as HostMapping,
        title: mapping.title_override,
        host: mapping.host,
        target: isProxyHostMapping(mapping)
          ? mapping.target
          : (mapping.static_serve?.path ?? ""),
      }));
    if (!rows.value.length) return;
    baseline = signature(options.allMappings.value);
    initialDraft = draftSignature();
    complete = onComplete;
    pendingGateway = [];
    mappingsSaved.value = false;
    error.value = "";
    attempted.value = false;
    discardOpen.value = false;
    open.value = true;
  };
  const assertUnchanged = () => {
    if (baseline !== signature(options.allMappings.value))
      throw new Error(t("conflict"));
  };
  const save = async () => {
    if (!open.value || busy.value || (!dirty.value && !mappingsSaved.value))
      return;
    attempted.value = true;
    if (
      !mappingsSaved.value &&
      errors.value.some((row) => row.host || row.target)
    )
      return;
    saving.value = true;
    error.value = "";
    try {
      assertUnchanged();
      if (!mappingsSaved.value) {
        const renames = new Map(
          rows.value
            .filter((row) => normalizeHostLike(row.host) !== row.originalHost)
            .map((row) => [row.originalHost, normalizeHostLike(row.host)]),
        );
        pendingGateway = [];
        if (renames.size) {
          for (const kind of [
            "gateway_proxy_headers",
            "gateway_host_response",
          ] as const) {
            const before = await options.readGatewayHosts(kind);
            const disabledBefore = new Set(normalizeDisabledHosts(before));
            let after = before;
            for (const row of rows.value) {
              const nextHost = renames.get(row.originalHost);
              if (!nextHost) continue;
              after = mergeGatewayDisabledHostsForMapping(
                after,
                [row.originalHost],
                isProxyHostMapping(row.original) ? nextHost : "",
                !disabledBefore.has(normalizeHostLike(row.originalHost)),
              );
            }
            if (!sameHosts(before, after))
              pendingGateway.push({
                kind,
                before,
                after,
                // Both catalog reconciliation and gateway GETs remove old hosts.
                afterCatalogCleanup: before.filter(
                  (host) => !renames.has(normalizeHostLike(host)),
                ),
              });
          }
        }
        assertUnchanged();
        const byHost = new Map(
          rows.value.map((row) => [row.originalHost, row]),
        );
        const next = options.allMappings.value.map((mapping) => {
          const row = byHost.get(mapping.host);
          if (!row) return mapping;
          const result = {
            ...mapping,
            host: normalizeHostLike(row.host),
            title_override: row.title.trim(),
          };
          if (isProxyHostMapping(mapping)) {
            result.target = row.target.trim();
            if (
              result.target !== mapping.target &&
              isWebSocketProxyTargetUrl(result.target)
            )
              result.suppress_toolbar = true;
          } else if (mapping.static_serve) {
            result.static_serve = {
              ...mapping.static_serve,
              path: row.target.trim(),
            };
          }
          return result;
        });
        await options.saveHostMappings(
          next,
          undefined,
          undefined,
          new Map([...renames].map(([before, after]) => [after, before])),
          assertUnchanged,
        );
        mappingsSaved.value = true;
        baseline = signature(options.allMappings.value);
      }
      // Keep successful steps out of retries, including the catalog save.
      while (pendingGateway.length) {
        const change = pendingGateway[0]!;
        const current = await options.readGatewayHosts(change.kind);
        assertUnchanged();
        if (
          !sameHosts(current, change.after) &&
          !sameHosts(current, change.before) &&
          !sameHosts(current, change.afterCatalogCleanup)
        )
          throw new Error(t("conflict"));
        // A persisted config does not prove runtime synchronization succeeded.
        // Retry every unacknowledged write, even if GET already shows its value.
        await options.writeGatewayHosts(change.kind, change.after);
        pendingGateway.shift();
      }
      complete?.();
      options.onSaved();
      close();
    } catch (cause) {
      const detail = extractErrorMessage(cause, t("failed"));
      error.value = mappingsSaved.value ? `${t("partial")} ${detail}` : detail;
      if (
        !mappingsSaved.value &&
        (cause as { response?: { status?: number } })?.response?.status === 409
      ) {
        error.value = t("conflict");
        try {
          // Refresh the catalog revision too; reopening a stale store would
          // otherwise repeat the same conflict indefinitely.
          await options.reloadMappings();
        } catch (refreshError) {
          error.value += ` ${extractErrorMessage(refreshError, t("failed"))}`;
        }
      }
    } finally {
      saving.value = false;
    }
  };
  return {
    open,
    rows,
    saving: busy,
    mappingsSaved,
    discardOpen,
    error,
    attempted,
    dirty,
    errors,
    openDialog,
    requestClose,
    discard,
    save,
  };
};
export type SubdomainBatchEditController = ReturnType<
  typeof useSubdomainBatchEdit
>;
