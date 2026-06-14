import { randomUUID } from "node:crypto";
import { redis } from "../redis";
import {
  DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  RedisLogBuffer,
} from "../redis-log-buffer";
import type {
  DDNSIpSource,
  DDNSLastCheck,
  DDNSLastIP,
  DDNSLogEntry,
  DDNSNetworkInterfaceOption,
  DDNSProviderDefinition,
  DDNSProviderField,
  DDNSSettings,
  DDNSStatus,
  DDNSTargetList,
  DDNSTargetMeta,
  DDNSTargetRecord,
  DDNSTargetSummary,
  DDNSUpdateResult,
  DDNSUpdateScope,
} from "./types";
import { providerDefinitions, providerUpdaters } from "./providers";
import { ensureEdgeOneOverseasAccessSynced } from "./providers/edgeone-overseas-access";
import {
  applyUpdateScope,
  ddnsTranslate,
  DDNS_UPDATE_SCOPE_FIELD,
  DEFAULT_DDNS_UPDATE_SCOPE,
  getUpdateScopeUnavailableMessage,
  normalizeUpdateScope,
  withDDNSLocale,
} from "./providers/helpers";
import {
  EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
  isEdgeOneDDNSProvider,
  normalizeEdgeOneOverseasAccessMode,
} from "./providers/edgeone-shared";
import {
  DDNS_INTERFACE_IPV4_INDEX_FIELD,
  DDNS_INTERFACE_IPV6_INDEX_FIELD,
  DDNS_IP_SOURCE_FIELD,
  DEFAULT_DDNS_IP_SOURCE,
  normalizeInterfaceAddressIndex,
  normalizeIpSource,
} from "./ip-source";
import {
  createDDNSHttpClient,
  DDNS_NETWORK_INTERFACE_FIELD,
  DEFAULT_DDNS_NETWORK_INTERFACE,
  findDDNSNetworkInterface,
  listDDNSNetworkInterfaces,
  normalizeNetworkInterface,
} from "./network";
import { runWithRetry } from "./retry";

const PRIMARY_TARGET_ID = "primary";
export const DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES = 10;
export const MIN_DDNS_UPDATE_INTERVAL_MINUTES = 5;
export const MAX_DDNS_UPDATE_INTERVAL_MINUTES = 1440;

const KEYS = {
  enabled: "fn_knock:ddns:enabled",
  settings: "fn_knock:ddns:settings",
  legacyProvider: "fn_knock:ddns:provider",
  legacyConfigPrefix: "fn_knock:ddns:config:",
  legacyLastIP: "fn_knock:ddns:last_ip",
  legacyLastCheck: "fn_knock:ddns:last_check",
  targetIds: "fn_knock:ddns:v2:target_ids",
  primaryTargetId: "fn_knock:ddns:v2:primary_target_id",
  targetPrefix: "fn_knock:ddns:v2:target:",
  logs: "fn_knock:ddns:logs",
  logSeq: "fn_knock:ddns:logs:seq",
} as const;

const LOG_TTL = 7 * 24 * 3600;
const ddnsT = ddnsTranslate;
const ddnsLogBuffer = new RedisLogBuffer(redis, {
  key: KEYS.logs,
  ttlSeconds: LOG_TTL,
  maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  seqKey: KEYS.logSeq,
});

const DDNS_PROVIDER_I18N_KEYS: Record<string, string> = {
  baiducloud: "baidu",
  huaweicloud: "huawei",
};

const getDDNSProviderI18nKey = (providerName: string): string =>
  DDNS_PROVIDER_I18N_KEYS[providerName] ?? providerName;

const translateDDNSCatalogText = (
  key: string,
  fallback: string | undefined,
  params?: Record<string, string | number | boolean | null | undefined>,
): string | undefined => {
  if (fallback === undefined) return undefined;
  const fullKey = `server.ddns.${key}`;
  const translated = ddnsT(key, params);
  return translated === fullKey ? fallback : translated;
};

const localizeProviderField = (
  providerKey: string,
  field: DDNSProviderField,
): DDNSProviderField => {
  const fieldParams =
    field.key === "ttl"
      ? {
          seconds: Number(field.placeholder) || 600,
        }
      : undefined;
  const localizeFieldPart = (
    part: "label" | "placeholder" | "description",
    fallback: string | undefined,
  ) => {
    const providerValue = translateDDNSCatalogText(
      `providers.${providerKey}.fields.${field.key}.${part}`,
      fallback,
      fieldParams,
    );
    if (providerValue !== fallback) return providerValue;
    return translateDDNSCatalogText(
      `providers.common.fields.${field.key}.${part}`,
      fallback,
      fieldParams,
    );
  };

  return {
    ...field,
    label: localizeFieldPart("label", field.label) ?? field.label,
    ...(field.placeholder !== undefined
      ? { placeholder: localizeFieldPart("placeholder", field.placeholder) }
      : {}),
    ...(field.description !== undefined
      ? { description: localizeFieldPart("description", field.description) }
      : {}),
    ...(field.options
      ? {
          options: field.options.map((option) => ({
            ...option,
            label:
              translateDDNSCatalogText(
                `providers.${providerKey}.fields.${field.key}.options.${option.value}`,
                option.label,
              ) ?? option.label,
          })),
        }
      : {}),
  };
};

const localizeProviderDefinition = (
  provider: DDNSProviderDefinition,
): DDNSProviderDefinition => {
  const providerKey = getDDNSProviderI18nKey(provider.name);
  return {
    ...provider,
    label:
      translateDDNSCatalogText(
        `providers.${providerKey}.label`,
        provider.label,
      ) ?? provider.label,
    fields: provider.fields.map((field) =>
      localizeProviderField(providerKey, field),
    ),
  };
};

const buildEmptyLastIP = (): DDNSLastIP => ({
  ipv4: null,
  ipv6: null,
  updated_at: null,
});

const buildEmptyLastCheck = (): DDNSLastCheck => ({
  checked_at: null,
  outcome: null,
  message: null,
});

const normalizeOutcome = (
  value: string | null | undefined,
): DDNSLastCheck["outcome"] => {
  return value === "updated" ||
    value === "noop" ||
    value === "skipped" ||
    value === "error"
    ? value
    : null;
};

const normalizeUpdateIntervalMinutes = (
  value: unknown,
  fallback = DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
): number => {
  const parsed =
    typeof value === "number"
      ? value
      : typeof value === "string"
        ? Number(value.trim())
        : NaN;

  if (
    Number.isInteger(parsed) &&
    parsed >= MIN_DDNS_UPDATE_INTERVAL_MINUTES &&
    parsed <= MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return parsed;
  }

  return fallback;
};

const parseUpdateIntervalMinutesInput = (value: unknown): number | null => {
  if (typeof value === "string" && !/^\d+$/.test(value.trim())) {
    return null;
  }

  const parsed = typeof value === "number" ? value : Number(value);
  if (
    Number.isInteger(parsed) &&
    parsed >= MIN_DDNS_UPDATE_INTERVAL_MINUTES &&
    parsed <= MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return parsed;
  }

  return null;
};

const parseLegacyDDNSCronIntervalMinutes = (
  pattern: string | null | undefined,
): number | null => {
  const parts = pattern?.trim().split(/\s+/).filter(Boolean) || [];
  if (parts.length !== 5 && parts.length !== 6) {
    return null;
  }

  const minutePart = parts.length === 6 ? parts[1] : parts[0];
  const otherParts = parts.length === 6 ? parts.slice(2) : parts.slice(1);
  if (!minutePart) {
    return null;
  }
  if (parts.length === 6 && parts[0] !== "0") {
    return null;
  }

  if (!otherParts.every((part) => part === "*")) {
    return null;
  }

  const match = minutePart.match(/^\*\/(\d+)$/);
  if (!match) {
    return null;
  }

  const interval = match[1];
  if (!interval) {
    return null;
  }

  const minutes = Number.parseInt(interval, 10);
  if (
    !Number.isInteger(minutes) ||
    minutes < MIN_DDNS_UPDATE_INTERVAL_MINUTES ||
    minutes > MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return null;
  }

  return minutes;
};

const getDefaultUpdateIntervalMinutes = () =>
  parseLegacyDDNSCronIntervalMinutes(process.env.DDNS_CRON) ??
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES;

const targetMetaKey = (id: string) => `${KEYS.targetPrefix}${id}:meta`;
const targetConfigKey = (id: string) => `${KEYS.targetPrefix}${id}:config`;
const targetLastIPKey = (id: string) => `${KEYS.targetPrefix}${id}:last_ip`;
const targetLastCheckKey = (id: string) =>
  `${KEYS.targetPrefix}${id}:last_check`;

export class DDNSManager {
  getProviders(locale?: string | null): DDNSProviderDefinition[] {
    return withDDNSLocale(locale, () =>
      providerDefinitions.map((provider) =>
        localizeProviderDefinition(provider),
      ),
    );
  }

  getProviderFields(name: string): DDNSProviderField[] | null {
    const provider = providerDefinitions.find((item) => item.name === name);
    return provider ? provider.fields : null;
  }

  async getSettings(): Promise<DDNSSettings> {
    const raw = await redis.get(KEYS.settings);
    if (!raw) {
      return {
        updateIntervalMinutes: getDefaultUpdateIntervalMinutes(),
      };
    }

    try {
      const parsed = JSON.parse(raw) as Partial<DDNSSettings>;
      return {
        updateIntervalMinutes: normalizeUpdateIntervalMinutes(
          parsed.updateIntervalMinutes,
        ),
      };
    } catch {
      return {
        updateIntervalMinutes: DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
      };
    }
  }

  async updateSettings(input: {
    updateIntervalMinutes: number;
  }): Promise<DDNSSettings> {
    const updateIntervalMinutes = parseUpdateIntervalMinutesInput(
      input.updateIntervalMinutes,
    );
    if (updateIntervalMinutes === null) {
      throw new Error(
        ddnsT("intervalOutOfRange", {
          min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
          max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
        }),
      );
    }

    await redis.set(KEYS.settings, JSON.stringify({ updateIntervalMinutes }));
    return this.getSettings();
  }

  private getProviderDefinition(
    name: string | null | undefined,
  ): DDNSProviderDefinition | null {
    const normalized = name?.trim() || "";
    if (!normalized) {
      return null;
    }
    return providerDefinitions.find((item) => item.name === normalized) || null;
  }

  private getProviderLabel(name: string | null | undefined): string {
    return (
      this.getProviderDefinition(name)?.label ||
      name?.trim() ||
      ddnsT("notConfigured")
    );
  }

  private normalizeConfig(
    providerName: string | null | undefined,
    config: Record<string, string> | null | undefined,
  ): Record<string, string> {
    const data = config || {};
    return {
      ...data,
      [DDNS_UPDATE_SCOPE_FIELD]: normalizeUpdateScope(
        data[DDNS_UPDATE_SCOPE_FIELD],
      ),
      [DDNS_IP_SOURCE_FIELD]: normalizeIpSource(data[DDNS_IP_SOURCE_FIELD]),
      [DDNS_NETWORK_INTERFACE_FIELD]: normalizeNetworkInterface(
        data[DDNS_NETWORK_INTERFACE_FIELD],
      ),
      [DDNS_INTERFACE_IPV4_INDEX_FIELD]: normalizeInterfaceAddressIndex(
        data[DDNS_INTERFACE_IPV4_INDEX_FIELD],
      ),
      [DDNS_INTERFACE_IPV6_INDEX_FIELD]: normalizeInterfaceAddressIndex(
        data[DDNS_INTERFACE_IPV6_INDEX_FIELD],
      ),
      ...(isEdgeOneDDNSProvider(providerName || "")
        ? {
            [EDGEONE_OVERSEAS_ACCESS_MODE_FIELD]:
              normalizeEdgeOneOverseasAccessMode(
                data[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD],
              ),
          }
        : {}),
    };
  }

  private prepareConfigForStorage(
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): Partial<Record<string, string>> {
    const normalizedProviderName = providerName?.trim() || "";
    const ipSource = normalizeIpSource(config[DDNS_IP_SOURCE_FIELD]);
    const normalizedConfig: Partial<Record<string, string>> = {
      ...config,
      [DDNS_UPDATE_SCOPE_FIELD]: normalizeUpdateScope(
        config[DDNS_UPDATE_SCOPE_FIELD],
      ),
      [DDNS_IP_SOURCE_FIELD]: ipSource,
      [DDNS_NETWORK_INTERFACE_FIELD]: normalizeNetworkInterface(
        config[DDNS_NETWORK_INTERFACE_FIELD],
      ),
      [DDNS_INTERFACE_IPV4_INDEX_FIELD]: normalizeInterfaceAddressIndex(
        config[DDNS_INTERFACE_IPV4_INDEX_FIELD],
      ),
      [DDNS_INTERFACE_IPV6_INDEX_FIELD]: normalizeInterfaceAddressIndex(
        config[DDNS_INTERFACE_IPV6_INDEX_FIELD],
      ),
      ...(isEdgeOneDDNSProvider(normalizedProviderName)
        ? {
            [EDGEONE_OVERSEAS_ACCESS_MODE_FIELD]:
              normalizeEdgeOneOverseasAccessMode(
                config[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD],
              ),
          }
        : {}),
    };

    if (ipSource === DEFAULT_DDNS_IP_SOURCE) {
      delete normalizedConfig[DDNS_IP_SOURCE_FIELD];
    }

    if (ipSource !== "interface") {
      delete normalizedConfig[DDNS_INTERFACE_IPV4_INDEX_FIELD];
      delete normalizedConfig[DDNS_INTERFACE_IPV6_INDEX_FIELD];
    } else {
      if (!normalizedConfig[DDNS_INTERFACE_IPV4_INDEX_FIELD]) {
        delete normalizedConfig[DDNS_INTERFACE_IPV4_INDEX_FIELD];
      }
      if (!normalizedConfig[DDNS_INTERFACE_IPV6_INDEX_FIELD]) {
        delete normalizedConfig[DDNS_INTERFACE_IPV6_INDEX_FIELD];
      }
    }

    if (
      !isEdgeOneDDNSProvider(normalizedProviderName) ||
      normalizedConfig[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD] === "off"
    ) {
      delete normalizedConfig[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD];
    }

    return normalizedConfig;
  }

  private parseLastIP(
    data: Record<string, string> | null | undefined,
  ): DDNSLastIP {
    return {
      ipv4: data?.ipv4 || null,
      ipv6: data?.ipv6 || null,
      updated_at: data?.updated_at || null,
    };
  }

  private parseLastCheck(
    data: Record<string, string> | null | undefined,
  ): DDNSLastCheck {
    return {
      checked_at: data?.checked_at || null,
      outcome: normalizeOutcome(data?.outcome),
      message: data?.message || null,
    };
  }

  private parseTargetMeta(
    id: string,
    data: Record<string, string> | null | undefined,
  ): DDNSTargetMeta | null {
    if (!data || Object.keys(data).length === 0) {
      return null;
    }

    const now = new Date().toISOString();
    const parsedSortOrder = Number(data.sort_order);

    return {
      id,
      name:
        data.name?.trim() ||
        (id === PRIMARY_TARGET_ID ? ddnsT("primaryDomainName") : ""),
      isPrimary: data.is_primary === "true" || id === PRIMARY_TARGET_ID,
      enabled: id === PRIMARY_TARGET_ID ? true : data.enabled !== "false",
      provider: data.provider?.trim() || null,
      createdAt: data.created_at || now,
      updatedAt: data.updated_at || data.created_at || now,
      sortOrder: Number.isFinite(parsedSortOrder)
        ? parsedSortOrder
        : id === PRIMARY_TARGET_ID
          ? 0
          : 1,
    };
  }

  private async getTargetMetaRaw(id: string): Promise<DDNSTargetMeta | null> {
    return this.parseTargetMeta(id, await redis.hgetall(targetMetaKey(id)));
  }

  private async saveTargetMeta(meta: DDNSTargetMeta): Promise<void> {
    const key = targetMetaKey(meta.id);
    const payload: Record<string, string> = {
      name: meta.name.trim(),
      is_primary: meta.isPrimary ? "true" : "false",
      enabled: meta.enabled ? "true" : "false",
      provider: meta.provider?.trim() || "",
      created_at: meta.createdAt,
      updated_at: meta.updatedAt,
      sort_order: String(meta.sortOrder),
    };

    await redis.del(key);
    await redis.hmset(key, payload);
    await redis.sadd(KEYS.targetIds, meta.id);
    if (meta.isPrimary) {
      await redis.set(KEYS.primaryTargetId, meta.id);
    }
  }

  private async getTargetConfigRaw(
    id: string,
    providerName: string | null | undefined,
  ): Promise<Record<string, string>> {
    return this.normalizeConfig(
      providerName,
      await redis.hgetall(targetConfigKey(id)),
    );
  }

  private async saveTargetConfigRaw(
    id: string,
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): Promise<void> {
    const key = targetConfigKey(id);
    const payload = this.prepareConfigForStorage(providerName, config);
    await redis.del(key);
    if (Object.keys(payload).length > 0) {
      await redis.hmset(key, payload as Record<string, string>);
    }
  }

  private async getTargetLastIPRaw(id: string): Promise<DDNSLastIP> {
    return this.parseLastIP(await redis.hgetall(targetLastIPKey(id)));
  }

  private async saveTargetLastIPRaw(
    id: string,
    status: DDNSLastIP,
  ): Promise<void> {
    const key = targetLastIPKey(id);
    const payload: Record<string, string> = {};

    if (status.ipv4) {
      payload.ipv4 = status.ipv4;
    }
    if (status.ipv6) {
      payload.ipv6 = status.ipv6;
    }
    if (status.updated_at) {
      payload.updated_at = status.updated_at;
    }

    await redis.del(key);
    if (Object.keys(payload).length > 0) {
      await redis.hmset(key, payload);
    }
  }

  private async getTargetLastCheckRaw(id: string): Promise<DDNSLastCheck> {
    return this.parseLastCheck(await redis.hgetall(targetLastCheckKey(id)));
  }

  private async saveTargetLastCheckRaw(
    id: string,
    status: DDNSLastCheck,
  ): Promise<void> {
    const key = targetLastCheckKey(id);
    const payload: Record<string, string> = {};

    if (status.checked_at) {
      payload.checked_at = status.checked_at;
    }
    if (status.outcome) {
      payload.outcome = status.outcome;
    }
    if (status.message) {
      payload.message = status.message;
    }

    await redis.del(key);
    if (Object.keys(payload).length > 0) {
      await redis.hmset(key, payload);
    }
  }

  private buildComparableConfigKey(
    providerName: string | null | undefined,
    config: Record<string, string> | null | undefined,
  ): string {
    const normalizedProviderName = providerName?.trim() || "";
    const prepared = this.prepareConfigForStorage(
      normalizedProviderName,
      this.normalizeConfig(normalizedProviderName, config || {}),
    );

    return JSON.stringify(
      Object.entries(prepared).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    );
  }

  private didTargetRuntimeInputsChange(
    current: Pick<DDNSTargetRecord, "provider" | "config">,
    next: {
      provider: string | null | undefined;
      config: Record<string, string> | null | undefined;
    },
  ): boolean {
    const currentProvider = current.provider?.trim() || "";
    const nextProvider = next.provider?.trim() || "";

    if (currentProvider !== nextProvider) {
      return true;
    }

    return (
      this.buildComparableConfigKey(currentProvider, current.config) !==
      this.buildComparableConfigKey(nextProvider, next.config)
    );
  }

  private async resetTargetRuntimeState(
    target: Pick<DDNSTargetMeta, "id" | "isPrimary">,
  ): Promise<void> {
    const emptyLastIP = buildEmptyLastIP();
    const emptyLastCheck = buildEmptyLastCheck();

    await Promise.all([
      this.saveTargetLastIPRaw(target.id, emptyLastIP),
      this.saveTargetLastCheckRaw(target.id, emptyLastCheck),
      ...(target.isPrimary
        ? [
            this.writeLegacyLastIP(emptyLastIP),
            this.writeLegacyLastCheck(emptyLastCheck),
          ]
        : []),
    ]);
  }

  private async readLegacyConfigDraft(
    providerName: string | null | undefined,
  ): Promise<Record<string, string>> {
    const normalizedProviderName = providerName?.trim() || "";
    if (!normalizedProviderName) {
      return this.normalizeConfig(null, {});
    }

    return this.normalizeConfig(
      normalizedProviderName,
      await redis.hgetall(KEYS.legacyConfigPrefix + normalizedProviderName),
    );
  }

  private async saveLegacyConfigDraft(
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): Promise<void> {
    const normalizedProviderName = providerName?.trim() || "";
    if (!normalizedProviderName) {
      return;
    }

    const key = KEYS.legacyConfigPrefix + normalizedProviderName;
    const payload = this.prepareConfigForStorage(
      normalizedProviderName,
      config,
    );

    await redis.del(key);
    if (Object.keys(payload).length > 0) {
      await redis.hmset(key, payload as Record<string, string>);
    }
  }

  private async readLegacyLastIP(): Promise<DDNSLastIP> {
    return this.parseLastIP(await redis.hgetall(KEYS.legacyLastIP));
  }

  private async writeLegacyLastIP(status: DDNSLastIP): Promise<void> {
    const payload: Record<string, string> = {};
    if (status.ipv4) {
      payload.ipv4 = status.ipv4;
    }
    if (status.ipv6) {
      payload.ipv6 = status.ipv6;
    }
    if (status.updated_at) {
      payload.updated_at = status.updated_at;
    }

    await redis.del(KEYS.legacyLastIP);
    if (Object.keys(payload).length > 0) {
      await redis.hmset(KEYS.legacyLastIP, payload);
    }
  }

  private async readLegacyLastCheck(): Promise<DDNSLastCheck> {
    return this.parseLastCheck(await redis.hgetall(KEYS.legacyLastCheck));
  }

  private async writeLegacyLastCheck(status: DDNSLastCheck): Promise<void> {
    const payload: Record<string, string> = {};
    if (status.checked_at) {
      payload.checked_at = status.checked_at;
    }
    if (status.outcome) {
      payload.outcome = status.outcome;
    }
    if (status.message) {
      payload.message = status.message;
    }

    await redis.del(KEYS.legacyLastCheck);
    if (Object.keys(payload).length > 0) {
      await redis.hmset(KEYS.legacyLastCheck, payload);
    }
  }

  private async mirrorPrimaryProvider(
    providerName: string | null | undefined,
  ): Promise<void> {
    const normalizedProviderName = providerName?.trim() || "";
    if (!normalizedProviderName) {
      await redis.del(KEYS.legacyProvider);
      return;
    }
    await redis.set(KEYS.legacyProvider, normalizedProviderName);
  }

  private async ensureTargetsInitialized(): Promise<void> {
    const currentPrimaryTargetId = await redis.get(KEYS.primaryTargetId);
    if (currentPrimaryTargetId) {
      const existing = await this.getTargetMetaRaw(currentPrimaryTargetId);
      if (existing) {
        await redis.sadd(KEYS.targetIds, currentPrimaryTargetId);
        return;
      }
    }

    const now = new Date().toISOString();
    const legacyProviderValue = (await redis.get(KEYS.legacyProvider))?.trim();
    const legacyProvider = this.getProviderDefinition(legacyProviderValue)?.name
      ? legacyProviderValue || null
      : null;
    const primaryMeta: DDNSTargetMeta = {
      id: PRIMARY_TARGET_ID,
      name: ddnsT("primaryDomainName"),
      isPrimary: true,
      enabled: true,
      provider: legacyProvider,
      createdAt: now,
      updatedAt: now,
      sortOrder: 0,
    };
    const primaryConfig = legacyProvider
      ? await this.readLegacyConfigDraft(legacyProvider)
      : this.normalizeConfig(null, {});

    await this.saveTargetMeta(primaryMeta);
    await this.saveTargetConfigRaw(
      primaryMeta.id,
      primaryMeta.provider,
      primaryConfig,
    );
    await this.saveTargetLastIPRaw(
      primaryMeta.id,
      await this.readLegacyLastIP(),
    );
    await this.saveTargetLastCheckRaw(
      primaryMeta.id,
      await this.readLegacyLastCheck(),
    );
    await this.mirrorPrimaryProvider(primaryMeta.provider);
  }

  private compareTargets(left: DDNSTargetMeta, right: DDNSTargetMeta): number {
    if (left.isPrimary !== right.isPrimary) {
      return left.isPrimary ? -1 : 1;
    }
    if (left.sortOrder !== right.sortOrder) {
      return left.sortOrder - right.sortOrder;
    }
    if (left.createdAt !== right.createdAt) {
      return left.createdAt.localeCompare(right.createdAt);
    }
    return left.id.localeCompare(right.id);
  }

  private buildDomainSummary(
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): string {
    const provider = providerName?.trim() || "";
    const candidates = [
      config.domain,
      config.hostname,
      config.domains,
      config.zone,
      config.root_domain,
      config.site_name,
      config.site_id,
    ];
    const summary = candidates.find((value) => value?.trim())?.trim() || "";

    if (summary) {
      return summary;
    }

    return provider ? "" : ddnsT("noProviderSelected");
  }

  private buildDisplayName(
    meta: DDNSTargetMeta,
    providerLabel: string,
    domainSummary: string,
  ): string {
    const explicitName = meta.name.trim();
    if (explicitName) {
      return explicitName;
    }
    if (meta.isPrimary) {
      return ddnsT("primaryDomainName");
    }
    return domainSummary || providerLabel;
  }

  private buildDuplicateKey(
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): string {
    const normalizedProviderName = providerName?.trim() || "";
    const normalizedDomainSummary = this.buildDomainSummary(
      normalizedProviderName,
      config,
    )
      .trim()
      .toLowerCase();

    if (!normalizedProviderName || !normalizedDomainSummary) {
      return "";
    }

    return `${normalizedProviderName}::${normalizedDomainSummary}`;
  }

  private async assertNoDuplicateTarget(
    providerName: string | null | undefined,
    config: Record<string, string>,
    excludeId?: string,
  ): Promise<void> {
    const duplicateKey = this.buildDuplicateKey(providerName, config);
    if (!duplicateKey) {
      return;
    }

    const targets = await this.listTargets();
    const duplicated = targets.find((target) => {
      if (excludeId && target.id === excludeId) {
        return false;
      }
      return (
        this.buildDuplicateKey(target.provider, target.config) === duplicateKey
      );
    });

    if (!duplicated) {
      return;
    }

    throw new Error(ddnsT("duplicateTarget"));
  }

  private async getPrimaryTargetMeta(): Promise<DDNSTargetMeta> {
    await this.ensureTargetsInitialized();
    const primaryTargetId =
      (await redis.get(KEYS.primaryTargetId)) || PRIMARY_TARGET_ID;
    const primaryTarget = await this.getTargetMetaRaw(primaryTargetId);
    if (!primaryTarget) {
      throw new Error(ddnsT("primaryInitFailed"));
    }
    return primaryTarget;
  }

  private async buildTargetRecordFromMeta(
    meta: DDNSTargetMeta,
  ): Promise<DDNSTargetRecord> {
    const [config, lastIP, lastCheck] = await Promise.all([
      this.getTargetConfigRaw(meta.id, meta.provider),
      this.getTargetLastIPRaw(meta.id),
      this.getTargetLastCheckRaw(meta.id),
    ]);

    return {
      ...meta,
      config,
      lastIP,
      lastCheck,
    };
  }

  private toTargetSummary(target: DDNSTargetRecord): DDNSTargetSummary {
    const providerLabel = this.getProviderLabel(target.provider);
    const domainSummary = this.buildDomainSummary(
      target.provider,
      target.config,
    );

    return {
      id: target.id,
      name: this.buildDisplayName(target, providerLabel, domainSummary),
      isPrimary: target.isPrimary,
      enabled: target.isPrimary ? true : target.enabled,
      provider: target.provider,
      updateScope: normalizeUpdateScope(target.config[DDNS_UPDATE_SCOPE_FIELD]),
      providerLabel,
      domainSummary,
      createdAt: target.createdAt,
      updatedAt: target.updatedAt,
      sortOrder: target.sortOrder,
      lastIP: target.lastIP,
      lastCheck: target.lastCheck,
    };
  }

  private getTargetLogLabel(
    target: Pick<
      DDNSTargetRecord | DDNSTargetSummary,
      "id" | "isPrimary" | "name" | "provider"
    >,
    config?: Record<string, string>,
  ): string {
    const providerLabel = this.getProviderLabel(target.provider);
    const domainSummary =
      "domainSummary" in target
        ? target.domainSummary
        : this.buildDomainSummary(target.provider, config || {});
    const label = domainSummary || target.name || providerLabel;
    const scope = target.isPrimary
      ? ddnsT("primaryDomainScope")
      : ddnsT("additionalDomainScope");
    return `[${scope}][${providerLabel}]${label ? `[${label}]` : ""}`;
  }

  async isEnabled(): Promise<boolean> {
    return (await redis.get(KEYS.enabled)) === "true";
  }

  async setEnabled(enabled: boolean): Promise<void> {
    await redis.set(KEYS.enabled, enabled ? "true" : "false");
  }

  async listTargets(): Promise<DDNSTargetRecord[]> {
    await this.ensureTargetsInitialized();

    const [primaryTargetId, rawIds] = await Promise.all([
      redis.get(KEYS.primaryTargetId),
      redis.smembers(KEYS.targetIds),
    ]);
    const ids = Array.from(
      new Set([...(primaryTargetId ? [primaryTargetId] : []), ...rawIds]),
    );
    const metas = (
      await Promise.all(ids.map((id) => this.getTargetMetaRaw(id)))
    ).filter((item): item is DDNSTargetMeta => item !== null);

    metas.sort((left, right) => this.compareTargets(left, right));

    return Promise.all(
      metas.map((meta) => this.buildTargetRecordFromMeta(meta)),
    );
  }

  async getTarget(id: string): Promise<DDNSTargetRecord | null> {
    await this.ensureTargetsInitialized();
    const meta = await this.getTargetMetaRaw(id);
    return meta ? this.buildTargetRecordFromMeta(meta) : null;
  }

  async getPrimaryTarget(): Promise<DDNSTargetRecord> {
    return this.buildTargetRecordFromMeta(await this.getPrimaryTargetMeta());
  }

  async getTargetConfig(targetId: string): Promise<Record<string, string>> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }
    return target.config;
  }

  async saveTargetConfig(
    targetId: string,
    config: Record<string, string>,
  ): Promise<void> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }

    const nextConfig = this.normalizeConfig(target.provider, config);
    const shouldResetRuntime = this.didTargetRuntimeInputsChange(target, {
      provider: target.provider,
      config: nextConfig,
    });

    await this.saveTargetConfigRaw(target.id, target.provider, nextConfig);
    if (shouldResetRuntime) {
      await this.resetTargetRuntimeState(target);
    }
    if (target.isPrimary) {
      await this.saveLegacyConfigDraft(target.provider, nextConfig);
    }
  }

  async getTargetLastIP(targetId: string): Promise<DDNSLastIP> {
    await this.ensureTargetsInitialized();
    return this.getTargetLastIPRaw(targetId);
  }

  async setTargetLastIP(
    targetId: string,
    ipv4: string | null,
    ipv6: string | null,
    options: { merge?: boolean } = {},
  ): Promise<void> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }

    const previous = options.merge ? target.lastIP : null;
    const next: DDNSLastIP = {
      ipv4: ipv4 ?? previous?.ipv4 ?? null,
      ipv6: ipv6 ?? previous?.ipv6 ?? null,
      updated_at: new Date().toISOString(),
    };
    await this.saveTargetLastIPRaw(target.id, next);
    if (target.isPrimary) {
      await this.writeLegacyLastIP(next);
    }
  }

  async getTargetLastCheck(targetId: string): Promise<DDNSLastCheck> {
    await this.ensureTargetsInitialized();
    return this.getTargetLastCheckRaw(targetId);
  }

  async setTargetLastCheck(
    targetId: string,
    outcome: NonNullable<DDNSLastCheck["outcome"]>,
    message: string,
  ): Promise<void> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }

    const next: DDNSLastCheck = {
      checked_at: new Date().toISOString(),
      outcome,
      message,
    };
    await this.saveTargetLastCheckRaw(target.id, next);
    if (target.isPrimary) {
      await this.writeLegacyLastCheck(next);
    }
  }

  async buildTargetSummary(
    targetId: string,
  ): Promise<DDNSTargetSummary | null> {
    const target = await this.getTarget(targetId);
    return target ? this.toTargetSummary(target) : null;
  }

  async getTargetsOverview(): Promise<DDNSTargetList> {
    const items = (await this.listTargets()).map((target) =>
      this.toTargetSummary(target),
    );
    const primaryTargetId = items.find((item) => item.isPrimary)?.id || null;
    const extras = items.filter((item) => !item.isPrimary);

    return {
      primaryTargetId,
      total: items.length,
      extraCount: extras.length,
      enabledExtraCount: extras.filter((item) => item.enabled).length,
      items,
    };
  }

  async createTarget(input: {
    name?: string;
    provider: string;
    enabled?: boolean;
    config?: Record<string, string>;
  }): Promise<DDNSTargetRecord> {
    await this.ensureTargetsInitialized();

    const providerName = this.getProviderDefinition(input.provider)?.name;
    if (!providerName) {
      throw new Error(ddnsT("unknownProvider", { provider: input.provider }));
    }

    const config = this.normalizeConfig(providerName, input.config || {});
    await this.assertNoDuplicateTarget(providerName, config);

    const now = new Date().toISOString();
    const currentTargets = await this.listTargets();
    const sortOrder =
      currentTargets.reduce(
        (max, target) => Math.max(max, target.sortOrder),
        0,
      ) + 1;
    const meta: DDNSTargetMeta = {
      id: randomUUID(),
      name: input.name?.trim() || "",
      isPrimary: false,
      enabled: input.enabled !== false,
      provider: providerName,
      createdAt: now,
      updatedAt: now,
      sortOrder,
    };

    await this.saveTargetMeta(meta);
    await this.saveTargetConfigRaw(meta.id, providerName, config);

    return this.buildTargetRecordFromMeta(meta);
  }

  async updateTarget(
    targetId: string,
    patch: {
      name?: string;
      enabled?: boolean;
      provider: string;
      config?: Record<string, string>;
    },
  ): Promise<DDNSTargetRecord> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }

    const providerName = this.getProviderDefinition(patch.provider)?.name;
    if (!providerName) {
      throw new Error(ddnsT("unknownProvider", { provider: patch.provider }));
    }

    const nextConfig = this.normalizeConfig(providerName, patch.config || {});
    await this.assertNoDuplicateTarget(providerName, nextConfig, target.id);
    const shouldResetRuntime = this.didTargetRuntimeInputsChange(target, {
      provider: providerName,
      config: nextConfig,
    });

    const nextMeta: DDNSTargetMeta = {
      ...target,
      name: patch.name === undefined ? target.name : patch.name.trim(),
      provider: providerName,
      enabled: target.isPrimary
        ? true
        : patch.enabled === undefined
          ? target.enabled
          : patch.enabled,
      updatedAt: new Date().toISOString(),
    };

    await this.saveTargetMeta(nextMeta);
    await this.saveTargetConfigRaw(nextMeta.id, providerName, nextConfig);
    if (shouldResetRuntime) {
      await this.resetTargetRuntimeState(nextMeta);
    }

    if (nextMeta.isPrimary) {
      await this.saveLegacyConfigDraft(providerName, nextConfig);
      await this.mirrorPrimaryProvider(providerName);
    }

    return this.buildTargetRecordFromMeta(nextMeta);
  }

  async deleteTarget(targetId: string): Promise<void> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }
    if (target.isPrimary) {
      throw new Error(ddnsT("primaryDeleteForbidden"));
    }

    await Promise.all([
      redis.srem(KEYS.targetIds, target.id),
      redis.del(targetMetaKey(target.id)),
      redis.del(targetConfigKey(target.id)),
      redis.del(targetLastIPKey(target.id)),
      redis.del(targetLastCheckKey(target.id)),
    ]);
  }

  async setTargetEnabled(targetId: string, enabled: boolean): Promise<void> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }
    if (target.isPrimary && !enabled) {
      throw new Error(ddnsT("primaryDisableForbidden"));
    }

    await this.saveTargetMeta({
      ...target,
      enabled: target.isPrimary ? true : enabled,
      updatedAt: new Date().toISOString(),
    });
  }

  async listRunnableTargets(): Promise<DDNSTargetRecord[]> {
    return (await this.listTargets()).filter(
      (target) => target.isPrimary || target.enabled,
    );
  }

  async getProvider(): Promise<string | null> {
    return (await this.getPrimaryTarget()).provider;
  }

  async setProvider(name: string): Promise<void> {
    const providerName = this.getProviderDefinition(name)?.name;
    if (!providerName) {
      throw new Error(ddnsT("unknownProvider", { provider: name }));
    }

    const primary = await this.getPrimaryTarget();
    if (primary.provider === providerName) {
      await this.mirrorPrimaryProvider(providerName);
      return;
    }

    if (primary.provider) {
      await this.saveLegacyConfigDraft(primary.provider, primary.config);
    }

    const nextConfig = await this.readLegacyConfigDraft(providerName);
    await this.assertNoDuplicateTarget(providerName, nextConfig, primary.id);
    const shouldResetRuntime = this.didTargetRuntimeInputsChange(primary, {
      provider: providerName,
      config: nextConfig,
    });
    const nextMeta: DDNSTargetMeta = {
      ...primary,
      provider: providerName,
      updatedAt: new Date().toISOString(),
      enabled: true,
    };

    await this.saveTargetMeta(nextMeta);
    await this.saveTargetConfigRaw(nextMeta.id, providerName, nextConfig);
    if (shouldResetRuntime) {
      await this.resetTargetRuntimeState(nextMeta);
    }
    await this.mirrorPrimaryProvider(providerName);
  }

  async getConfig(providerName: string): Promise<Record<string, string>> {
    const primary = await this.getPrimaryTarget();
    return primary.provider === providerName ? primary.config : {};
  }

  async saveConfig(
    providerName: string,
    config: Record<string, string>,
  ): Promise<void> {
    const normalizedProviderName =
      this.getProviderDefinition(providerName)?.name;
    if (!normalizedProviderName) {
      throw new Error(ddnsT("unknownProvider", { provider: providerName }));
    }

    const primary = await this.getPrimaryTarget();
    if (primary.provider === normalizedProviderName) {
      const nextConfig = this.normalizeConfig(normalizedProviderName, config);
      await this.assertNoDuplicateTarget(
        normalizedProviderName,
        nextConfig,
        primary.id,
      );
      const shouldResetRuntime = this.didTargetRuntimeInputsChange(primary, {
        provider: normalizedProviderName,
        config: nextConfig,
      });
      await this.saveTargetConfigRaw(
        primary.id,
        normalizedProviderName,
        nextConfig,
      );
      if (shouldResetRuntime) {
        await this.resetTargetRuntimeState(primary);
      }
      await this.saveLegacyConfigDraft(normalizedProviderName, nextConfig);
      return;
    }

    await this.saveLegacyConfigDraft(normalizedProviderName, config);
  }

  async getLastIP(): Promise<DDNSLastIP> {
    return (await this.getPrimaryTarget()).lastIP;
  }

  async setLastIP(
    ipv4: string | null,
    ipv6: string | null,
    options: { merge?: boolean } = {},
  ): Promise<void> {
    await this.setTargetLastIP(PRIMARY_TARGET_ID, ipv4, ipv6, options);
  }

  async getUpdateScope(providerName?: string | null): Promise<DDNSUpdateScope> {
    const primary = await this.getPrimaryTarget();
    const config =
      providerName && primary.provider !== providerName ? {} : primary.config;
    return normalizeUpdateScope(config[DDNS_UPDATE_SCOPE_FIELD]);
  }

  async getIpSource(providerName?: string | null): Promise<DDNSIpSource> {
    const primary = await this.getPrimaryTarget();
    const config =
      providerName && primary.provider !== providerName ? {} : primary.config;
    return normalizeIpSource(config[DDNS_IP_SOURCE_FIELD]);
  }

  async getNetworkInterface(providerName?: string | null): Promise<string> {
    const primary = await this.getPrimaryTarget();
    const config =
      providerName && primary.provider !== providerName ? {} : primary.config;
    return normalizeNetworkInterface(config[DDNS_NETWORK_INTERFACE_FIELD]);
  }

  async getLastCheck(): Promise<DDNSLastCheck> {
    return (await this.getPrimaryTarget()).lastCheck;
  }

  async setLastCheck(
    outcome: NonNullable<DDNSLastCheck["outcome"]>,
    message: string,
  ): Promise<void> {
    await this.setTargetLastCheck(PRIMARY_TARGET_ID, outcome, message);
  }

  async getStatus(): Promise<DDNSStatus> {
    const [enabled, primaryTarget, overview, settings] = await Promise.all([
      this.isEnabled(),
      this.getPrimaryTarget(),
      this.getTargetsOverview(),
      this.getSettings(),
    ]);

    return {
      enabled,
      provider: primaryTarget.provider,
      updateIntervalMinutes: settings.updateIntervalMinutes,
      updateScope: normalizeUpdateScope(
        primaryTarget.config[DDNS_UPDATE_SCOPE_FIELD],
      ),
      ipSource: normalizeIpSource(primaryTarget.config[DDNS_IP_SOURCE_FIELD]),
      networkInterface: normalizeNetworkInterface(
        primaryTarget.config[DDNS_NETWORK_INTERFACE_FIELD],
      ),
      lastIP: primaryTarget.lastIP,
      lastCheck: primaryTarget.lastCheck,
      primaryTargetId: overview.primaryTargetId,
      extraTargetCount: overview.extraCount,
      enabledExtraTargetCount: overview.enabledExtraCount,
      targets: overview.items,
    };
  }

  listNetworkInterfaces(): DDNSNetworkInterfaceOption[] {
    return listDDNSNetworkInterfaces();
  }

  async appendLog(
    level: DDNSLogEntry["level"],
    message: string,
    context: Partial<
      Pick<DDNSLogEntry, "targetId" | "targetName" | "provider" | "isPrimary">
    > = {},
  ): Promise<void> {
    const entry: DDNSLogEntry = {
      time: new Date().toISOString(),
      level,
      message,
      ...(context.targetId ? { targetId: context.targetId } : {}),
      ...(context.targetName ? { targetName: context.targetName } : {}),
      ...("provider" in context ? { provider: context.provider ?? null } : {}),
      ...(typeof context.isPrimary === "boolean"
        ? { isPrimary: context.isPrimary }
        : {}),
    };
    await ddnsLogBuffer.append([JSON.stringify(entry)]);
  }

  async appendTargetLog(
    level: DDNSLogEntry["level"],
    target: DDNSTargetRecord | DDNSTargetSummary,
    message: string,
  ): Promise<void> {
    await this.appendLog(
      level,
      `${this.getTargetLogLabel(target, "config" in target ? target.config : {})} ${message}`,
      {
        targetId: target.id,
        targetName: target.name,
        provider: target.provider,
        isPrimary: target.isPrimary,
      },
    );
  }

  async getLogs(limit: number = 200): Promise<DDNSLogEntry[]> {
    const raw = await ddnsLogBuffer.list(limit);
    return raw.map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return { time: "", level: "info", message: line };
      }
    });
  }

  async clearLogs(): Promise<void> {
    await ddnsLogBuffer.clear();
  }

  private async ensureProviderAuxiliaryStateWithContext(
    providerName: string,
    config: Record<string, string>,
    http = createDDNSHttpClient({
      networkInterface: config[DDNS_NETWORK_INTERFACE_FIELD],
    }),
  ): Promise<{ changed: boolean; message: string | null }> {
    if (!isEdgeOneDDNSProvider(providerName)) {
      return { changed: false, message: null };
    }

    const result = await ensureEdgeOneOverseasAccessSynced({
      providerName,
      context: {
        config,
        http,
      },
    });

    return {
      changed: result.changed,
      message: result.message || null,
    };
  }

  async ensureProviderAuxiliaryState(
    options: {
      emitLog?: boolean;
      logPrefix?: string;
      providerName?: string | null;
    } = {},
  ): Promise<void> {
    const primary = await this.getPrimaryTarget();
    if (!primary.provider) {
      return;
    }

    const result = await this.ensureProviderAuxiliaryStateWithContext(
      primary.provider,
      primary.config,
    );

    if (options.emitLog && result.changed && result.message) {
      await this.appendTargetLog(
        "info",
        this.toTargetSummary(primary),
        options.logPrefix
          ? `${options.logPrefix}: ${result.message}`
          : result.message,
      );
    }
  }

  async ensureTargetAuxiliaryState(
    targetOrId: string | DDNSTargetRecord,
    options: {
      emitLog?: boolean;
      logPrefix?: string;
    } = {},
  ): Promise<void> {
    const target =
      typeof targetOrId === "string"
        ? await this.getTarget(targetOrId)
        : targetOrId;
    if (!target?.provider) {
      return;
    }

    const result = await this.ensureProviderAuxiliaryStateWithContext(
      target.provider,
      target.config,
    );

    if (options.emitLog && result.changed && result.message) {
      await this.appendTargetLog(
        "info",
        this.toTargetSummary(target),
        options.logPrefix
          ? `${options.logPrefix}: ${result.message}`
          : result.message,
      );
    }
  }

  async executeTargetUpdate(
    targetOrId: string | DDNSTargetRecord,
    ipv4: string | null,
    ipv6: string | null,
    locale?: string | null,
  ): Promise<DDNSUpdateResult> {
    return withDDNSLocale(locale, async () => {
      const target =
        typeof targetOrId === "string"
          ? await this.getTarget(targetOrId)
          : targetOrId;

      if (!target) {
        return { success: false, message: ddnsT("targetNotFound") };
      }
      if (!target.provider) {
        return { success: false, message: ddnsT("noProviderSelected") };
      }

      const updater = providerUpdaters[target.provider];
      if (!updater) {
        return {
          success: false,
          message: ddnsT("unknownProviderShort", { provider: target.provider }),
        };
      }

      const http = createDDNSHttpClient({
        networkInterface: target.config[DDNS_NETWORK_INTERFACE_FIELD],
      });
      const updateScope = normalizeUpdateScope(
        target.config[DDNS_UPDATE_SCOPE_FIELD],
      );
      const scopedIPs = applyUpdateScope(updateScope, ipv4, ipv6);

      if (!scopedIPs.ipv4 && !scopedIPs.ipv6) {
        return {
          success: false,
          message: getUpdateScopeUnavailableMessage(updateScope),
        };
      }

      const retryCount = Number(process.env.DDNS_RETRY_COUNT || "1");
      const maxAttempts = Math.max(1, retryCount + 1);
      const delayMs = Number(process.env.DDNS_RETRY_DELAY_MS || "600");

      try {
        await this.ensureProviderAuxiliaryStateWithContext(
          target.provider,
          target.config,
          http,
        );

        return await runWithRetry(
          () =>
            updater(
              { config: target.config, http },
              scopedIPs.ipv4,
              scopedIPs.ipv6,
            ),
          { maxAttempts, delayMs },
        );
      } catch (error: any) {
        return {
          success: false,
          message: error?.message || String(error),
        };
      }
    });
  }

  async executeUpdate(
    ipv4: string | null,
    ipv6: string | null,
    locale?: string | null,
  ): Promise<DDNSUpdateResult> {
    return this.executeTargetUpdate(PRIMARY_TARGET_ID, ipv4, ipv6, locale);
  }

  async isTargetConfigComplete(
    targetOrId: string | DDNSTargetRecord,
  ): Promise<boolean> {
    const target =
      typeof targetOrId === "string"
        ? await this.getTarget(targetOrId)
        : targetOrId;
    if (!target?.provider) {
      return false;
    }

    const definition = this.getProviderDefinition(target.provider);
    if (!definition) {
      return false;
    }

    const requiredFields = definition.fields.filter(
      (field) => field.required !== false,
    );
    const providerFieldsComplete = requiredFields.every(
      (field) => !!target.config[field.key],
    );
    if (!providerFieldsComplete) {
      return false;
    }

    const ipSource = normalizeIpSource(target.config[DDNS_IP_SOURCE_FIELD]);
    if (ipSource !== "interface") {
      return true;
    }

    const networkInterface = normalizeNetworkInterface(
      target.config[DDNS_NETWORK_INTERFACE_FIELD],
    );
    if (!networkInterface) {
      return false;
    }

    const network = findDDNSNetworkInterface(networkInterface);
    if (!network) {
      return false;
    }

    const updateScope = normalizeUpdateScope(
      target.config[DDNS_UPDATE_SCOPE_FIELD],
    );
    const requiresIPv4 = updateScope !== "ipv6_only";
    const requiresIPv6 = updateScope !== "ipv4_only";
    const hasSelectableIPv4 = network.selectableAddresses.some(
      (item) => item.family === "ipv4",
    );
    const hasSelectableIPv6 = network.selectableAddresses.some(
      (item) => item.family === "ipv6",
    );

    if (
      requiresIPv4 &&
      hasSelectableIPv4 &&
      !normalizeInterfaceAddressIndex(
        target.config[DDNS_INTERFACE_IPV4_INDEX_FIELD],
      )
    ) {
      return false;
    }

    if (
      requiresIPv6 &&
      hasSelectableIPv6 &&
      !normalizeInterfaceAddressIndex(
        target.config[DDNS_INTERFACE_IPV6_INDEX_FIELD],
      )
    ) {
      return false;
    }

    return true;
  }

  async isConfigComplete(): Promise<boolean> {
    return this.isTargetConfigComplete(PRIMARY_TARGET_ID);
  }
}

export const ddnsManager = new DDNSManager();
export { ddnsLogBuffer };

export type {
  DDNSLastCheck,
  DDNSIpSource,
  DDNSLastIP,
  DDNSLogEntry,
  DDNSNetworkInterfaceOption,
  DDNSProviderDefinition,
  DDNSProviderField,
  DDNSSettings,
  DDNSStatus,
  DDNSTargetList,
  DDNSTargetMeta,
  DDNSTargetRecord,
  DDNSTargetSummary,
  DDNSUpdateResult,
  DDNSUpdateScope,
} from "./types";
