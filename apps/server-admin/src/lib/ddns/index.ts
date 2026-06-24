import { randomUUID } from "node:crypto";
import { redis } from "../redis";
import {
  DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  RedisLogBuffer,
} from "../redis-log-buffer";
import type {
  DDNSIpSource,
  DDNSHttpTransport,
  DDNSLastCheck,
  DDNSLastIP,
  DDNSLogEntry,
  DDNSNetworkInterfaceOption,
  DDNSPublicCheckSources,
  DDNSProviderDefinition,
  DDNSProviderField,
  DDNSSettings,
  DDNSStoredSettings,
  DDNSStatus,
  DDNSTargetList,
  DDNSTargetMeta,
  DDNSTargetRecord,
  DDNSTargetSummary,
  DDNSUpdateResult,
  DDNSUpdateScope,
} from "./types";
import { providerDefinitions, providerUpdaters } from "./providers";
import { localizeProviderDefinition } from "./catalog";
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
import { isEdgeOneDDNSProvider } from "./providers/edgeone-shared";
import {
  DDNS_IP_SOURCE_FIELD,
  normalizeIpSource,
} from "./ip-source";
import {
  createDDNSHttpClient,
  DDNS_NETWORK_INTERFACE_FIELD,
  listDDNSNetworkInterfaces,
  normalizeNetworkInterface,
} from "./network";
import { runWithRetry } from "./retry";
import {
  buildComparableDDNSConfigKey,
  normalizeDDNSConfig,
} from "./config-normalizer";
import {
  buildEmptyDDNSLastCheck,
  buildEmptyDDNSLastIP,
} from "./status-codec";
import {
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
  parseUpdateIntervalMinutesInput,
} from "./update-interval";
import {
  DDNS_REDIS_KEYS,
  DDNSRedisStore,
  PRIMARY_DDNS_TARGET_ID,
} from "./redis-store";
import { normalizeDDNSPublicCheckSources } from "./public-check-sources";
import {
  normalizeDDNSHttpTransport,
  parseDDNSSettingsRaw,
} from "./settings";
import {
  buildDDNSTargetDuplicateKey,
  buildDDNSTargetLogLabel,
  compareDDNSTargets,
  toDDNSTargetSummary,
} from "./target-view";
import { isDDNSTargetConfigComplete } from "./config-completeness";

export {
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
};

const PRIMARY_TARGET_ID = PRIMARY_DDNS_TARGET_ID;

const LOG_TTL = 7 * 24 * 3600;
const ddnsT = ddnsTranslate;
const ddnsLogBuffer = new RedisLogBuffer(redis, {
  key: DDNS_REDIS_KEYS.logs,
  ttlSeconds: LOG_TTL,
  maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  seqKey: DDNS_REDIS_KEYS.logSeq,
});

export class DDNSManager {
  private readonly store: DDNSRedisStore;

  constructor() {
    this.store = new DDNSRedisStore(redis);
  }

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
    const raw = await this.store.getSettingsRaw();
    return parseDDNSSettingsRaw(raw);
  }

  async updateSettings(input: {
    updateIntervalMinutes?: number;
    publicCheckSources?: DDNSPublicCheckSources;
    httpTransport?: DDNSHttpTransport;
  }): Promise<DDNSSettings> {
    const current = await this.getSettings();
    const updateIntervalMinutes =
      typeof input.updateIntervalMinutes === "undefined"
        ? current.updateIntervalMinutes
        : parseUpdateIntervalMinutesInput(input.updateIntervalMinutes);

    if (updateIntervalMinutes === null) {
      throw new Error(
        ddnsT("intervalOutOfRange", {
          min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
          max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
        }),
      );
    }

    const publicCheckSources =
      typeof input.publicCheckSources === "undefined"
        ? current.publicCheckSources
        : normalizeDDNSPublicCheckSources(
            input.publicCheckSources,
            current.publicCheckSources,
          );

    const settingsToSave: DDNSStoredSettings = {
      updateIntervalMinutes,
      publicCheckSources,
      httpTransport:
        typeof input.httpTransport === "undefined"
          ? current.httpTransport
          : normalizeDDNSHttpTransport(input.httpTransport),
    };
    await this.store.saveSettings(settingsToSave);
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

  private async getTargetMetaRaw(id: string): Promise<DDNSTargetMeta | null> {
    return this.store.getTargetMeta(id, ddnsT("primaryDomainName"));
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
      buildComparableDDNSConfigKey(currentProvider, current.config) !==
      buildComparableDDNSConfigKey(nextProvider, next.config)
    );
  }

  private async resetTargetRuntimeState(
    target: Pick<DDNSTargetMeta, "id" | "isPrimary">,
  ): Promise<void> {
    const emptyLastIP = buildEmptyDDNSLastIP();
    const emptyLastCheck = buildEmptyDDNSLastCheck();

    await Promise.all([
      this.store.saveTargetLastIP(target.id, emptyLastIP),
      this.store.saveTargetLastCheck(target.id, emptyLastCheck),
      ...(target.isPrimary
        ? [
            this.store.writeLegacyLastIP(emptyLastIP),
            this.store.writeLegacyLastCheck(emptyLastCheck),
          ]
        : []),
    ]);
  }

  private async ensureTargetsInitialized(): Promise<void> {
    const currentPrimaryTargetId = await this.store.getPrimaryTargetId();
    if (currentPrimaryTargetId) {
      const existing = await this.getTargetMetaRaw(currentPrimaryTargetId);
      if (existing) {
        await this.store.addTargetId(currentPrimaryTargetId);
        return;
      }
    }

    const now = new Date().toISOString();
    const legacyProviderValue = await this.store.readLegacyProvider();
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
      ? await this.store.readLegacyConfigDraft(legacyProvider)
      : normalizeDDNSConfig(null, {});

    await this.store.saveTargetMeta(primaryMeta);
    await this.store.saveTargetConfig(
      primaryMeta.id,
      primaryMeta.provider,
      primaryConfig,
    );
    await this.store.saveTargetLastIP(
      primaryMeta.id,
      await this.store.readLegacyLastIP(),
    );
    await this.store.saveTargetLastCheck(
      primaryMeta.id,
      await this.store.readLegacyLastCheck(),
    );
    await this.store.mirrorPrimaryProvider(primaryMeta.provider);
  }

  private async assertNoDuplicateTarget(
    providerName: string | null | undefined,
    config: Record<string, string>,
    excludeId?: string,
  ): Promise<void> {
    const duplicateKey = buildDDNSTargetDuplicateKey(providerName, config);
    if (!duplicateKey) {
      return;
    }

    const targets = await this.listTargets();
    const duplicated = targets.find((target) => {
      if (excludeId && target.id === excludeId) {
        return false;
      }
      return (
        buildDDNSTargetDuplicateKey(target.provider, target.config) ===
        duplicateKey
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
      (await this.store.getPrimaryTargetId()) || PRIMARY_TARGET_ID;
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
      this.store.getTargetConfig(meta.id, meta.provider),
      this.store.getTargetLastIP(meta.id),
      this.store.getTargetLastCheck(meta.id),
    ]);

    return {
      ...meta,
      config,
      lastIP,
      lastCheck,
    };
  }

  async isEnabled(): Promise<boolean> {
    return this.store.getEnabled();
  }

  async setEnabled(enabled: boolean): Promise<void> {
    await this.store.setEnabled(enabled);
  }

  async listTargets(): Promise<DDNSTargetRecord[]> {
    await this.ensureTargetsInitialized();

    const [primaryTargetId, rawIds] = await Promise.all([
      this.store.getPrimaryTargetId(),
      this.store.listTargetIds(),
    ]);
    const ids = Array.from(
      new Set([...(primaryTargetId ? [primaryTargetId] : []), ...rawIds]),
    );
    const metas = (
      await Promise.all(ids.map((id) => this.getTargetMetaRaw(id)))
    ).filter((item): item is DDNSTargetMeta => item !== null);

    metas.sort(compareDDNSTargets);

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

    const nextConfig = normalizeDDNSConfig(target.provider, config);
    const shouldResetRuntime = this.didTargetRuntimeInputsChange(target, {
      provider: target.provider,
      config: nextConfig,
    });

    await this.store.saveTargetConfig(target.id, target.provider, nextConfig);
    if (shouldResetRuntime) {
      await this.resetTargetRuntimeState(target);
    }
    if (target.isPrimary) {
      await this.store.saveLegacyConfigDraft(target.provider, nextConfig);
    }
  }

  async getTargetLastIP(targetId: string): Promise<DDNSLastIP> {
    await this.ensureTargetsInitialized();
    return this.store.getTargetLastIP(targetId);
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
    await this.store.saveTargetLastIP(target.id, next);
    if (target.isPrimary) {
      await this.store.writeLegacyLastIP(next);
    }
  }

  async getTargetLastCheck(targetId: string): Promise<DDNSLastCheck> {
    await this.ensureTargetsInitialized();
    return this.store.getTargetLastCheck(targetId);
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
    await this.store.saveTargetLastCheck(target.id, next);
    if (target.isPrimary) {
      await this.store.writeLegacyLastCheck(next);
    }
  }

  async buildTargetSummary(
    targetId: string,
  ): Promise<DDNSTargetSummary | null> {
    const target = await this.getTarget(targetId);
    return target
      ? toDDNSTargetSummary(target, (provider) =>
          this.getProviderLabel(provider),
        )
      : null;
  }

  async getTargetsOverview(): Promise<DDNSTargetList> {
    const items = (await this.listTargets()).map((target) =>
      toDDNSTargetSummary(target, (provider) =>
        this.getProviderLabel(provider),
      ),
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

    const config = normalizeDDNSConfig(providerName, input.config || {});
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

    await this.store.saveTargetMeta(meta);
    await this.store.saveTargetConfig(meta.id, providerName, config);

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

    const nextConfig = normalizeDDNSConfig(providerName, patch.config || {});
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

    await this.store.saveTargetMeta(nextMeta);
    await this.store.saveTargetConfig(nextMeta.id, providerName, nextConfig);
    if (shouldResetRuntime) {
      await this.resetTargetRuntimeState(nextMeta);
    }

    if (nextMeta.isPrimary) {
      await this.store.saveLegacyConfigDraft(providerName, nextConfig);
      await this.store.mirrorPrimaryProvider(providerName);
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

    await this.store.deleteTarget(target.id);
  }

  async setTargetEnabled(targetId: string, enabled: boolean): Promise<void> {
    const target = await this.getTarget(targetId);
    if (!target) {
      throw new Error(ddnsT("targetNotFound"));
    }
    if (target.isPrimary && !enabled) {
      throw new Error(ddnsT("primaryDisableForbidden"));
    }

    await this.store.saveTargetMeta({
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
      await this.store.mirrorPrimaryProvider(providerName);
      return;
    }

    if (primary.provider) {
      await this.store.saveLegacyConfigDraft(primary.provider, primary.config);
    }

    const nextConfig = await this.store.readLegacyConfigDraft(providerName);
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

    await this.store.saveTargetMeta(nextMeta);
    await this.store.saveTargetConfig(nextMeta.id, providerName, nextConfig);
    if (shouldResetRuntime) {
      await this.resetTargetRuntimeState(nextMeta);
    }
    await this.store.mirrorPrimaryProvider(providerName);
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
      const nextConfig = normalizeDDNSConfig(normalizedProviderName, config);
      await this.assertNoDuplicateTarget(
        normalizedProviderName,
        nextConfig,
        primary.id,
      );
      const shouldResetRuntime = this.didTargetRuntimeInputsChange(primary, {
        provider: normalizedProviderName,
        config: nextConfig,
      });
      await this.store.saveTargetConfig(
        primary.id,
        normalizedProviderName,
        nextConfig,
      );
      if (shouldResetRuntime) {
        await this.resetTargetRuntimeState(primary);
      }
      await this.store.saveLegacyConfigDraft(normalizedProviderName, nextConfig);
      return;
    }

    await this.store.saveLegacyConfigDraft(normalizedProviderName, config);
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
      publicCheckSources: settings.publicCheckSources,
      defaultPublicCheckSources: settings.defaultPublicCheckSources,
      httpTransport: settings.httpTransport,
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
      `${buildDDNSTargetLogLabel(
        target,
        (provider) => this.getProviderLabel(provider),
        "config" in target ? target.config : {},
      )} ${message}`,
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
    const settings = await this.getSettings();

    const result = await this.ensureProviderAuxiliaryStateWithContext(
      primary.provider,
      primary.config,
      createDDNSHttpClient({
        networkInterface: primary.config[DDNS_NETWORK_INTERFACE_FIELD],
        transport: settings.httpTransport,
      }),
    );

    if (options.emitLog && result.changed && result.message) {
      await this.appendTargetLog(
        "info",
        toDDNSTargetSummary(primary, (provider) =>
          this.getProviderLabel(provider),
        ),
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
    const settings = await this.getSettings();

    const result = await this.ensureProviderAuxiliaryStateWithContext(
      target.provider,
      target.config,
      createDDNSHttpClient({
        networkInterface: target.config[DDNS_NETWORK_INTERFACE_FIELD],
        transport: settings.httpTransport,
      }),
    );

    if (options.emitLog && result.changed && result.message) {
      await this.appendTargetLog(
        "info",
        toDDNSTargetSummary(target, (provider) =>
          this.getProviderLabel(provider),
        ),
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

      const settings = await this.getSettings();
      const http = createDDNSHttpClient({
        networkInterface: target.config[DDNS_NETWORK_INTERFACE_FIELD],
        transport: settings.httpTransport,
      });
      const definition = this.getProviderDefinition(target.provider);
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
      if (
        definition?.capabilities?.addressMode === "single_address" &&
        scopedIPs.ipv4 &&
        scopedIPs.ipv6
      ) {
        return {
          success: false,
          message: ddnsT("singleAddressProviderUnsupported", {
            provider: this.getProviderLabel(target.provider),
          }),
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
    return isDDNSTargetConfigComplete(target, definition);
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
  DDNSPublicCheckSources,
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
