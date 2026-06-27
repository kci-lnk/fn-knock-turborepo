import { type AcmeCertificateAuthority } from "../acme-certificate-authority";
import { type TerminalFeatureConfig } from "../terminal-shared";
import type { SSHSecurityConfig } from "../ssh-security/types";
import {
  normalizeReverseProxySubmode,
  type ReverseProxySubmode,
} from "../reverse-proxy-submode";
import { normalizeAutoManageFirewall } from "../firewall-automation";
import type { AutoHttpsConfig } from "../auto-https-redirect";
import {
  type LocaleConfig,
  normalizeLocaleConfig,
} from "../../../../../packages/i18n/src";
import type { AppearanceConfig } from "../../../../../packages/admin-shared/src/utils/appearance";
import {
  AcmeCertificateStore,
  type AcmeCertificatePair,
} from "./acme-certificate-store";
import { AcmeApplicationService } from "./acme-application-service";
import { AcmeDataStore } from "./acme-data-store";
import { AcmeLibraryService } from "./acme-library-service";
import { AcmeRuntimeStore } from "./acme-runtime-store";
import { AcmeSettingsStore } from "./acme-settings-store";
import { AuthCredentialStore } from "./auth-store";
import { CaHostStore } from "./ca-host-store";
import {
  createDefaultAppConfig,
  normalizePersistedAppConfig,
} from "./config-loader";
import {
  LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY,
  LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY,
  applyLegacyEventSystemResourceAlertsPatchIfNeeded,
  applyLegacyReverseProxyThrottlePatchIfNeeded,
} from "./legacy-patches";
import {
  DEFAULT_ROUTE_PLACEHOLDER,
  normalizeHostMappings,
  normalizeSmartConnectConfig,
  normalizeStreamMappings,
  normalizeSubdomainModeConfig,
} from "./app-config";
import { redis } from "./client";
import { redisT } from "./messages";
import { SSLConfigStore, type SaveSSLCertificateInput } from "./ssl-store";
import { ConfigSectionStore } from "./section-store";
import { OnboardingStore } from "./onboarding-store";
import { RedisEphemeralStore } from "./redis-ephemeral-store";
import { ConfigRuntimeStateStore } from "./runtime-state-store";
import { ConfigFeatureSectionService } from "./feature-section-service";
import { applyRuntimeConfigConstraints } from "./runtime-constraints";
import { buildSafeAppConfig } from "./safe-config";
import type {
  ProxyMapping,
  RunType,
  WelcomeGuideStatus,
  HostMapping,
  StreamMapping,
  SubdomainModeConfig,
  SSLConfig,
  SSLCertInfo,
  SSLCertificateSource,
  SSLManagedCertificate,
  SSLStatus,
  FnosShareBypassConfig,
  FnosPortIconHijackConfig,
  GatewayLoggingSettings,
  WAFConfig,
  ReverseProxyThrottleConfig,
  GatewayVisibilityConfig,
  GatewayVisibilityRuntimeState,
  GatewayProxyHeadersConfig,
  GatewayProxyHeadersRuntimeState,
  GatewayHostResponseConfig,
  GatewayHostResponseRuntimeState,
  GatewayCrawlerBlockerConfig,
  GatewayPortalConfig,
  ReverseProxyTrustedIPRuntimeState,
  ProtocolMappingFeatureConfig,
  DashboardDisplayConfig,
  SmartConnectConfig,
  ScanDiscoveryConfig,
  SmartConnectRuntimeState,
  CaptchaSettings,
  IpLocationApiConfig,
  AcmeJob,
  AcmeApplication,
  AcmeIssuedCertificate,
  AcmeRuntimeLock,
  AcmeApplicationSaveResult,
  AcmeApplicationDeleteResult,
  AcmeSettings,
  AcmeClientSettings,
  LoginSession,
  AppConfig,
  RunModePromptPreferences,
  AuthCredentialSettings,
  TOTPCredential,
  PasskeyCredential,
} from "./types";

export class ConfigManager {
  private redis: typeof redis;
  private acmeApplicationService: AcmeApplicationService;
  private acmeCertificateStore: AcmeCertificateStore;
  private acmeLibraryService: AcmeLibraryService;
  private acmeRuntimeStore: AcmeRuntimeStore;
  private acmeSettingsStore: AcmeSettingsStore;
  private authCredentialStore: AuthCredentialStore;
  private caHostStore: CaHostStore;
  private ephemeralStore: RedisEphemeralStore;
  private onboardingStore: OnboardingStore;
  private featureSections: ConfigFeatureSectionService;
  private sslStore: SSLConfigStore;
  private configKey = "fn_knock:config";
  private reverseProxyThrottlePatchFlagKey =
    LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY;
  private eventSystemResourceAlertsPatchFlagKey =
    LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY;
  constructor() {
    this.redis = redis;
    const acmeDataStore = new AcmeDataStore(this.redis);
    this.acmeRuntimeStore = new AcmeRuntimeStore(this.redis);
    this.acmeSettingsStore = new AcmeSettingsStore(this.redis);
    this.authCredentialStore = new AuthCredentialStore(this.redis);
    this.caHostStore = new CaHostStore(this.redis);
    this.ephemeralStore = new RedisEphemeralStore(this.redis);
    this.onboardingStore = new OnboardingStore(this.redis);
    const sections = new ConfigSectionStore(this, this.redis);
    this.featureSections = new ConfigFeatureSectionService(
      this,
      sections,
      new ConfigRuntimeStateStore(sections),
    );
    this.sslStore = new SSLConfigStore({
      getConfig: () => this.getConfig(),
      saveConfig: (config) => this.saveConfig(config),
    });
    this.acmeCertificateStore = new AcmeCertificateStore(this.redis, (cert) =>
      this.parseCertInfo(cert),
    );
    this.acmeLibraryService = new AcmeLibraryService({
      access: this,
      acmeCertificateStore: this.acmeCertificateStore,
      acmeDataStore,
      lookups: {
        getApplication: (id) => this.getAcmeApplication(id),
        getApplicationByPrimaryDomain: (primaryDomain) =>
          this.getAcmeApplicationByPrimaryDomain(primaryDomain),
        getIssuedCertificate: (applicationId) =>
          this.getAcmeIssuedCertificate(applicationId),
        linkIssuedCertificateToLibrary: (applicationId, libraryCertificateId) =>
          this.linkAcmeIssuedCertificateToLibrary(
            applicationId,
            libraryCertificateId,
          ),
      },
      sslStore: this.sslStore,
    });
    this.acmeApplicationService = new AcmeApplicationService({
      acmeCertificateStore: this.acmeCertificateStore,
      acmeDataStore,
      acmeLibraryService: this.acmeLibraryService,
      acmeSettingsStore: this.acmeSettingsStore,
      parseCertInfo: (cert) => this.parseCertInfo(cert),
      redis: this.redis,
      sslStore: this.sslStore,
    });
  }

  getAcmeRuntimeLockTtlSeconds(): number {
    return this.acmeRuntimeStore.getAcmeRuntimeLockTtlSeconds();
  }

  isAcmeIssuedCertificateCompatible(
    application:
      | Pick<AcmeApplication, "domains" | "primaryDomain">
      | null
      | undefined,
    issuedCertificate:
      | Pick<AcmeIssuedCertificate, "primaryDomain" | "certInfo">
      | null
      | undefined,
  ): boolean {
    return this.acmeLibraryService.isIssuedCertificateCompatible(
      application,
      issuedCertificate,
    );
  }

  async getUsableAcmeIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    return this.acmeLibraryService.getUsableIssuedCertificate(applicationId);
  }

  async applyLegacyReverseProxyThrottlePatchIfNeeded(): Promise<{
    applied: boolean;
    config: AppConfig;
  }> {
    return applyLegacyReverseProxyThrottlePatchIfNeeded({
      redis: this.redis,
      configKey: this.configKey,
      patchFlagKey: this.reverseProxyThrottlePatchFlagKey,
      getConfig: () => this.getConfig(),
    });
  }

  async applyLegacyEventSystemResourceAlertsPatchIfNeeded(): Promise<{
    applied: boolean;
    config: AppConfig;
  }> {
    return applyLegacyEventSystemResourceAlertsPatchIfNeeded({
      redis: this.redis,
      configKey: this.configKey,
      patchFlagKey: this.eventSystemResourceAlertsPatchFlagKey,
      getConfig: () => this.getConfig(),
    });
  }

  async getConfig(): Promise<AppConfig> {
    try {
      const data = await this.redis.get(this.configKey);
      if (data) {
        return normalizePersistedAppConfig(JSON.parse(data) as AppConfig);
      }
    } catch (e) {
      console.error("Failed to parse config from redis", e);
    }
    return createDefaultAppConfig();
  }

  /**
   * 返回不含 SSL cert/key 原文的配置（供 /api/admin/config 使用）
   */
  async getConfigSafe(): Promise<any> {
    const [config, protocolMappingFeature] = await Promise.all([
      this.getConfig(),
      this.getProtocolMappingFeatureConfig(),
    ]);
    return buildSafeAppConfig(config, protocolMappingFeature);
  }

  async getLocaleConfig(): Promise<LocaleConfig> {
    const config = await this.getConfig();
    return normalizeLocaleConfig(config.locale);
  }

  async updateLocaleConfig(
    patch: Partial<LocaleConfig>,
  ): Promise<LocaleConfig> {
    const config = await this.getConfig();
    const next = normalizeLocaleConfig({
      ...config.locale,
      ...patch,
    });
    await this.saveConfig({
      ...config,
      locale: next,
    });
    return next;
  }

  async applyRuntimeConstraints(): Promise<{
    updated: boolean;
    config: AppConfig;
    corrected: string[];
  }> {
    return applyRuntimeConfigConstraints({
      getConfig: () => this.getConfig(),
      saveConfig: (config) => this.saveConfig(config),
    });
  }

  private parseCertInfo(certPem: string): SSLCertInfo | null {
    return this.sslStore.parseCertInfo(certPem);
  }

  async getSSLStatus(): Promise<SSLStatus> {
    return this.sslStore.getSSLStatus();
  }

  validateSSLCert(
    cert: string,
    key: string,
  ): { valid: boolean; error?: string } {
    return this.sslStore.validateSSLCert(cert, key);
  }

  async clearSSL(): Promise<void> {
    await this.sslStore.clearSSL();
  }

  async clearSSLCertificateLibrary(): Promise<number> {
    return this.sslStore.clearSSLCertificateLibrary();
  }

  async saveConfig(config: AppConfig): Promise<void> {
    await this.redis.set(this.configKey, JSON.stringify(config));
  }

  async getSSLCertificate(id: string): Promise<SSLManagedCertificate | null> {
    return this.sslStore.getSSLCertificate(id);
  }

  async getActiveSSLCertificate(): Promise<SSLManagedCertificate | null> {
    return this.sslStore.getActiveSSLCertificate();
  }

  async saveSSLCertificate(
    input: SaveSSLCertificateInput,
  ): Promise<SSLManagedCertificate> {
    return this.sslStore.saveSSLCertificate(input);
  }

  async saveAcmeCertificateToLibrary(
    domain: string,
    opts?: {
      id?: string;
      label?: string;
      activate?: boolean;
    },
  ): Promise<SSLManagedCertificate> {
    return this.acmeLibraryService.saveCertificateToLibrary(domain, opts);
  }

  async getSSLCertificateBySourceRef(
    source: SSLCertificateSource,
    sourceRefId: string,
  ): Promise<SSLManagedCertificate | null> {
    return this.sslStore.getSSLCertificateBySourceRef(source, sourceRefId);
  }

  async activateSSLCertificate(
    id: string | null | undefined,
  ): Promise<SSLManagedCertificate | null> {
    return this.sslStore.activateSSLCertificate(id);
  }

  async deleteSSLCertificate(id: string): Promise<{
    removed: SSLManagedCertificate | null;
    removedActive: boolean;
  }> {
    return this.sslStore.deleteSSLCertificate(id);
  }

  async deleteSSLCertificatesBySource(
    source: SSLCertificateSource,
    primaryDomain?: string,
  ): Promise<{
    removed: SSLManagedCertificate[];
    removedActive: boolean;
  }> {
    return this.sslStore.deleteSSLCertificatesBySource(source, primaryDomain);
  }

  async deleteSSLCertificatesBySourceRef(
    source: SSLCertificateSource,
    sourceRefId: string,
  ): Promise<{
    removed: SSLManagedCertificate[];
    removedActive: boolean;
  }> {
    return this.sslStore.deleteSSLCertificatesBySourceRef(source, sourceRefId);
  }

  async ensureAcmeDataMigrated(): Promise<void> {
    await this.acmeApplicationService.ensureDataMigrated();
  }

  async listAcmeApplications(): Promise<AcmeApplication[]> {
    return this.acmeApplicationService.listApplications();
  }

  async getAcmeApplication(id: string): Promise<AcmeApplication | null> {
    return this.acmeApplicationService.getApplication(id);
  }

  async getAcmeApplicationByPrimaryDomain(
    primaryDomain: string,
  ): Promise<AcmeApplication | null> {
    return this.acmeApplicationService.getApplicationByPrimaryDomain(
      primaryDomain,
    );
  }

  async saveAcmeApplication(input: {
    id?: string;
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  }): Promise<AcmeApplication> {
    return this.acmeApplicationService.saveApplication(input);
  }

  async deleteAcmeApplication(
    id: string,
  ): Promise<AcmeApplicationDeleteResult | null> {
    return this.acmeApplicationService.deleteApplication(id);
  }

  async saveAcmeApplicationWithEffects(input: {
    id?: string;
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  }): Promise<AcmeApplicationSaveResult> {
    return this.acmeApplicationService.saveApplicationWithEffects(input);
  }

  async updateAcmeApplicationJobState(
    applicationId: string,
    job: Pick<
      AcmeJob,
      | "id"
      | "status"
      | "trigger"
      | "createdAt"
      | "startedAt"
      | "finishedAt"
      | "message"
    >,
  ): Promise<AcmeApplication | null> {
    return this.acmeApplicationService.updateApplicationJobState(
      applicationId,
      job,
    );
  }

  async listAcmeIssuedCertificates(): Promise<AcmeIssuedCertificate[]> {
    return this.acmeApplicationService.listIssuedCertificates();
  }

  async getAcmeIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    return this.acmeApplicationService.getIssuedCertificate(applicationId);
  }

  async getAcmeIssuedCertificateByPrimaryDomain(
    primaryDomain: string,
  ): Promise<AcmeIssuedCertificate | null> {
    return this.acmeApplicationService.getIssuedCertificateByPrimaryDomain(
      primaryDomain,
    );
  }

  async saveAcmeIssuedCertificate(input: {
    applicationId: string;
    primaryDomain: string;
    cert: string;
    key: string;
    certInfo: SSLCertInfo;
    libraryCertificateId?: string;
  }): Promise<AcmeIssuedCertificate> {
    return this.acmeApplicationService.saveIssuedCertificate(input);
  }

  async linkAcmeIssuedCertificateToLibrary(
    applicationId: string,
    libraryCertificateId?: string | null,
  ): Promise<AcmeIssuedCertificate | null> {
    return this.acmeApplicationService.linkIssuedCertificateToLibrary(
      applicationId,
      libraryCertificateId,
    );
  }

  async deleteAcmeIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    return this.acmeApplicationService.deleteIssuedCertificate(applicationId);
  }

  async saveAcmeIssuedCertFromFS(
    applicationId: string,
    primaryDomain: string,
    opts?: { forceInstall?: boolean },
  ): Promise<boolean> {
    return this.acmeApplicationService.saveIssuedCertFromFS(
      applicationId,
      primaryDomain,
      opts,
    );
  }

  async getAcmeRuntimeLock(): Promise<AcmeRuntimeLock> {
    return this.acmeRuntimeStore.getAcmeRuntimeLock();
  }

  async tryAcquireAcmeRuntimeLock(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): Promise<AcmeRuntimeLock | null> {
    return this.acmeRuntimeStore.tryAcquireAcmeRuntimeLock(lock, ttlSeconds);
  }

  async refreshAcmeRuntimeLock(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): Promise<AcmeRuntimeLock | null> {
    return this.acmeRuntimeStore.refreshAcmeRuntimeLock(lock, ttlSeconds);
  }

  async setAcmeRuntimeLock(lock: AcmeRuntimeLock): Promise<AcmeRuntimeLock> {
    return this.acmeRuntimeStore.setAcmeRuntimeLock(lock);
  }

  async releaseAcmeRuntimeLock(
    lock: AcmeRuntimeLock | string | null | undefined,
  ): Promise<boolean> {
    return this.acmeRuntimeStore.releaseAcmeRuntimeLock(lock);
  }

  async clearAcmeRuntimeLock(): Promise<void> {
    await this.acmeRuntimeStore.clearAcmeRuntimeLock();
  }

  async getActiveAcmeRuntimeLock(): Promise<AcmeRuntimeLock> {
    return this.acmeRuntimeStore.getActiveAcmeRuntimeLock();
  }

  async getActiveAcmeJobFromLock(): Promise<AcmeJob | null> {
    return this.acmeRuntimeStore.getActiveAcmeJobFromLock();
  }

  async saveAcmeCertificateToLibraryByApplication(
    applicationId: string,
    opts?: {
      id?: string;
      label?: string;
      activate?: boolean;
    },
  ): Promise<SSLManagedCertificate> {
    return this.acmeLibraryService.saveCertificateToLibraryByApplication(
      applicationId,
      opts,
    );
  }

  async createAcmeJob(job: AcmeJob): Promise<void> {
    await this.acmeRuntimeStore.createAcmeJob(job);
  }

  async updateAcmeJob(id: string, patch: Partial<AcmeJob>): Promise<void> {
    await this.acmeRuntimeStore.updateAcmeJob(id, patch);
  }

  async getAcmeJob(id: string): Promise<AcmeJob | null> {
    return this.acmeRuntimeStore.getAcmeJob(id);
  }

  async appendAcmeLog(jobId: string, line: string): Promise<void> {
    await this.acmeRuntimeStore.appendAcmeLog(jobId, line);
  }

  async clearAcmeLogs(jobId: string): Promise<void> {
    await this.acmeRuntimeStore.clearAcmeLogs(jobId);
  }

  async getAcmeLogs(
    jobId: string,
    limit: number = 500,
    order: "asc" | "desc" = "asc",
  ): Promise<string[]> {
    return this.acmeRuntimeStore.getAcmeLogs(jobId, limit, order);
  }

  async saveAcmeSettings(
    value: Omit<AcmeSettings, "updatedAt">,
  ): Promise<AcmeSettings> {
    return this.acmeApplicationService.saveSettings(value);
  }

  async getAcmeSettings(): Promise<AcmeSettings | null> {
    return this.acmeApplicationService.getSettings();
  }

  async saveAcmeClientSettings(
    value: Pick<AcmeClientSettings, "certificateAuthority">,
  ): Promise<AcmeClientSettings> {
    return this.acmeSettingsStore.saveClientSettings(value);
  }

  async getAcmeClientSettings(): Promise<AcmeClientSettings | null> {
    return this.acmeSettingsStore.getClientSettings();
  }

  async ensureAcmeClientSettings(
    fallbackCertificateAuthority?: AcmeCertificateAuthority,
  ): Promise<AcmeClientSettings> {
    return this.acmeSettingsStore.ensureClientSettings(
      fallbackCertificateAuthority,
    );
  }

  async saveAcmeCert(
    domain: string,
    cert: string,
    keyPem: string,
  ): Promise<void> {
    return this.acmeCertificateStore.save(domain, cert, keyPem);
  }

  async getAcmeCert(domain: string): Promise<AcmeCertificatePair | null> {
    return this.acmeCertificateStore.get(domain);
  }

  async deleteAcmeCert(domain: string): Promise<void> {
    return this.acmeCertificateStore.delete(domain);
  }

  async getAcmeCertInfo(domain: string): Promise<SSLCertInfo | null> {
    return this.acmeCertificateStore.getInfo(domain);
  }

  async saveAcmeCertFromFS(
    domain: string,
    opts?: { forceInstall?: boolean },
  ): Promise<boolean> {
    return this.acmeCertificateStore.saveFromFS(domain, opts);
  }

  async updateRunType(
    run_type: RunType,
    reverse_proxy_submode?: ReverseProxySubmode,
  ): Promise<void> {
    const config = await this.getConfig();
    config.run_type = run_type;
    if (run_type === 1 && reverse_proxy_submode !== undefined) {
      config.reverse_proxy_submode = normalizeReverseProxySubmode(
        reverse_proxy_submode,
      );
    }

    if (run_type === 3) {
      config.proxy_mappings = [];
      config.default_route = DEFAULT_ROUTE_PLACEHOLDER;
    }

    config.smart_connect = normalizeSmartConnectConfig(config.smart_connect);
    if (run_type !== 3) {
      config.smart_connect.enabled = false;
    }

    await this.saveConfig(config);
  }

  async updateAutoManageFirewall(
    auto_manage_firewall: boolean,
  ): Promise<boolean> {
    const config = await this.getConfig();
    config.auto_manage_firewall =
      normalizeAutoManageFirewall(auto_manage_firewall);
    await this.saveConfig(config);
    return config.auto_manage_firewall;
  }

  async updateReverseProxySubmode(
    reverse_proxy_submode: ReverseProxySubmode,
  ): Promise<void> {
    const config = await this.getConfig();
    config.reverse_proxy_submode = normalizeReverseProxySubmode(
      reverse_proxy_submode,
    );
    await this.saveConfig(config);
  }

  async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
    return this.onboardingStore.getRunModePromptPreferences();
  }

  async updateRunModePromptPreferences(
    patch: Partial<RunModePromptPreferences>,
  ): Promise<RunModePromptPreferences> {
    return this.onboardingStore.updateRunModePromptPreferences(patch);
  }

  async getWelcomeGuideStatus(): Promise<WelcomeGuideStatus> {
    return this.onboardingStore.getWelcomeGuideStatus();
  }

  async completeWelcomeGuide(): Promise<WelcomeGuideStatus> {
    return this.onboardingStore.completeWelcomeGuide();
  }

  async getProtocolMappingFeatureConfig(): Promise<ProtocolMappingFeatureConfig> {
    return this.featureSections.getProtocolMappingFeatureConfig();
  }

  async updateProtocolMappingFeatureConfig(
    patch: Partial<ProtocolMappingFeatureConfig>,
  ): Promise<ProtocolMappingFeatureConfig> {
    return this.featureSections.updateProtocolMappingFeatureConfig(patch);
  }

  async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
    return this.featureSections.getFnosShareBypassConfig();
  }

  async getFnosPortIconHijackConfig(): Promise<FnosPortIconHijackConfig> {
    return this.featureSections.getFnosPortIconHijackConfig();
  }

  async getGatewayLoggingConfig(): Promise<GatewayLoggingSettings> {
    return this.featureSections.getGatewayLoggingConfig();
  }

  async getWAFConfig(): Promise<WAFConfig> {
    return this.featureSections.getWAFConfig();
  }

  async getReverseProxyThrottleConfig(): Promise<ReverseProxyThrottleConfig> {
    return this.featureSections.getReverseProxyThrottleConfig();
  }

  async getGatewayVisibilityConfig(): Promise<GatewayVisibilityConfig> {
    return this.featureSections.getGatewayVisibilityConfig();
  }

  async getGatewayProxyHeadersConfig(): Promise<GatewayProxyHeadersConfig> {
    return this.featureSections.getGatewayProxyHeadersConfig();
  }

  async getGatewayHostResponseConfig(): Promise<GatewayHostResponseConfig> {
    return this.featureSections.getGatewayHostResponseConfig();
  }

  async getGatewayCrawlerBlockerConfig(): Promise<GatewayCrawlerBlockerConfig> {
    return this.featureSections.getGatewayCrawlerBlockerConfig();
  }

  async getGatewayPortalConfig(): Promise<GatewayPortalConfig> {
    return this.featureSections.getGatewayPortalConfig();
  }

  async getAppearanceConfig(): Promise<AppearanceConfig> {
    return this.featureSections.getAppearanceConfig();
  }

  async getDashboardDisplayConfig(): Promise<DashboardDisplayConfig> {
    return this.featureSections.getDashboardDisplayConfig();
  }

  async getAutoHttpsConfig(): Promise<AutoHttpsConfig> {
    return this.featureSections.getAutoHttpsConfig();
  }

  async getGatewayVisibilityRuntimeState(): Promise<GatewayVisibilityRuntimeState> {
    return this.featureSections.getGatewayVisibilityRuntimeState();
  }

  async getGatewayProxyHeadersRuntimeState(): Promise<GatewayProxyHeadersRuntimeState> {
    return this.featureSections.getGatewayProxyHeadersRuntimeState();
  }

  async getGatewayHostResponseRuntimeState(): Promise<GatewayHostResponseRuntimeState> {
    return this.featureSections.getGatewayHostResponseRuntimeState();
  }

  async getReverseProxyTrustedIPsRuntimeState(): Promise<ReverseProxyTrustedIPRuntimeState> {
    return this.featureSections.getReverseProxyTrustedIPsRuntimeState();
  }

  async getSmartConnectConfig(): Promise<SmartConnectConfig> {
    return this.featureSections.getSmartConnectConfig();
  }

  async updateSmartConnectConfig(
    patch: Partial<SmartConnectConfig>,
  ): Promise<SmartConnectConfig> {
    return this.featureSections.updateSmartConnectConfig(patch);
  }

  async getScanDiscoveryConfig(): Promise<ScanDiscoveryConfig> {
    return this.featureSections.getScanDiscoveryConfig();
  }

  async updateScanDiscoveryConfig(
    patch: Partial<ScanDiscoveryConfig>,
  ): Promise<ScanDiscoveryConfig> {
    return this.featureSections.updateScanDiscoveryConfig(patch);
  }

  async getSmartConnectRuntimeState(): Promise<SmartConnectRuntimeState> {
    return this.featureSections.getSmartConnectRuntimeState();
  }

  async saveSmartConnectRuntimeState(
    nextValue: SmartConnectRuntimeState,
  ): Promise<SmartConnectRuntimeState> {
    return this.featureSections.saveSmartConnectRuntimeState(nextValue);
  }

  async updateFnosShareBypassConfig(
    patch: Partial<FnosShareBypassConfig>,
  ): Promise<FnosShareBypassConfig> {
    return this.featureSections.updateFnosShareBypassConfig(patch);
  }

  async updateFnosPortIconHijackConfig(
    patch: Partial<FnosPortIconHijackConfig>,
  ): Promise<FnosPortIconHijackConfig> {
    return this.featureSections.updateFnosPortIconHijackConfig(patch);
  }

  async updateGatewayLoggingConfig(
    patch: Partial<GatewayLoggingSettings>,
  ): Promise<GatewayLoggingSettings> {
    return this.featureSections.updateGatewayLoggingConfig(patch);
  }

  async updateWAFConfig(patch: Partial<WAFConfig>): Promise<WAFConfig> {
    return this.featureSections.updateWAFConfig(patch);
  }

  async updateReverseProxyThrottleConfig(
    patch: Partial<ReverseProxyThrottleConfig>,
  ): Promise<ReverseProxyThrottleConfig> {
    return this.featureSections.updateReverseProxyThrottleConfig(patch);
  }

  async updateGatewayVisibilityConfig(
    nextValue: GatewayVisibilityConfig,
  ): Promise<GatewayVisibilityConfig> {
    return this.featureSections.updateGatewayVisibilityConfig(nextValue);
  }

  async updateGatewayProxyHeadersConfig(
    nextValue: GatewayProxyHeadersConfig,
  ): Promise<GatewayProxyHeadersConfig> {
    return this.featureSections.updateGatewayProxyHeadersConfig(nextValue);
  }

  async updateGatewayHostResponseConfig(
    nextValue: GatewayHostResponseConfig,
  ): Promise<GatewayHostResponseConfig> {
    return this.featureSections.updateGatewayHostResponseConfig(nextValue);
  }

  async updateGatewayCrawlerBlockerConfig(
    patch: Partial<GatewayCrawlerBlockerConfig>,
  ): Promise<GatewayCrawlerBlockerConfig> {
    return this.featureSections.updateGatewayCrawlerBlockerConfig(patch);
  }

  async updateGatewayPortalConfig(
    patch: Partial<GatewayPortalConfig>,
  ): Promise<GatewayPortalConfig> {
    return this.featureSections.updateGatewayPortalConfig(patch);
  }

  async updateAppearanceConfig(
    patch: Partial<AppearanceConfig>,
  ): Promise<AppearanceConfig> {
    return this.featureSections.updateAppearanceConfig(patch);
  }

  async updateDashboardDisplayConfig(
    patch: Partial<DashboardDisplayConfig>,
  ): Promise<DashboardDisplayConfig> {
    return this.featureSections.updateDashboardDisplayConfig(patch);
  }

  async updateAutoHttpsConfig(
    patch: Partial<AutoHttpsConfig>,
  ): Promise<AutoHttpsConfig> {
    return this.featureSections.updateAutoHttpsConfig(patch);
  }

  async saveGatewayVisibilityRuntimeState(
    nextValue: GatewayVisibilityRuntimeState,
  ): Promise<GatewayVisibilityRuntimeState> {
    return this.featureSections.saveGatewayVisibilityRuntimeState(nextValue);
  }

  async saveGatewayProxyHeadersRuntimeState(
    nextValue: GatewayProxyHeadersRuntimeState,
  ): Promise<GatewayProxyHeadersRuntimeState> {
    return this.featureSections.saveGatewayProxyHeadersRuntimeState(nextValue);
  }

  async saveGatewayHostResponseRuntimeState(
    nextValue: GatewayHostResponseRuntimeState,
  ): Promise<GatewayHostResponseRuntimeState> {
    return this.featureSections.saveGatewayHostResponseRuntimeState(nextValue);
  }

  async saveReverseProxyTrustedIPsRuntimeState(
    nextValue: ReverseProxyTrustedIPRuntimeState,
  ): Promise<ReverseProxyTrustedIPRuntimeState> {
    return this.featureSections.saveReverseProxyTrustedIPsRuntimeState(
      nextValue,
    );
  }

  async getTerminalFeatureConfig(): Promise<TerminalFeatureConfig> {
    return this.featureSections.getTerminalFeatureConfig();
  }

  async getSSHSecurityConfig(): Promise<SSHSecurityConfig> {
    return this.featureSections.getSSHSecurityConfig();
  }

  async getAuthCredentialSettings(): Promise<AuthCredentialSettings> {
    return this.featureSections.getAuthCredentialSettings();
  }

  async previewAuthCredentialSettingsUpdate(
    patch: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    return this.featureSections.previewAuthCredentialSettingsUpdate(patch);
  }

  async updateAuthCredentialSettings(
    patch: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    return this.featureSections.updateAuthCredentialSettings(patch);
  }

  async updateTerminalFeatureConfig(
    patch: Partial<TerminalFeatureConfig>,
  ): Promise<TerminalFeatureConfig> {
    return this.featureSections.updateTerminalFeatureConfig(patch);
  }

  async updateSSHSecurityConfig(
    nextValue: SSHSecurityConfig,
  ): Promise<SSHSecurityConfig> {
    return this.featureSections.updateSSHSecurityConfig(nextValue);
  }

  async getCaptchaSettings(): Promise<CaptchaSettings> {
    return this.featureSections.getCaptchaSettings();
  }

  async updateCaptchaSettings(
    patch: Partial<CaptchaSettings>,
  ): Promise<CaptchaSettings> {
    return this.featureSections.updateCaptchaSettings(patch);
  }

  async getIpLocationApiSettings(): Promise<IpLocationApiConfig> {
    return this.featureSections.getIpLocationApiSettings();
  }

  async updateIpLocationApiSettings(
    patch: Partial<IpLocationApiConfig>,
  ): Promise<IpLocationApiConfig> {
    return this.featureSections.updateIpLocationApiSettings(patch);
  }

  async updateProxyMappings(mappings: ProxyMapping[]): Promise<void> {
    const config = await this.getConfig();
    config.proxy_mappings = mappings;
    await this.saveConfig(config);
  }

  async updateHostMappings(
    mappings: Array<Partial<HostMapping>>,
  ): Promise<void> {
    const config = await this.getConfig();
    config.host_mappings = normalizeHostMappings(mappings);
    await this.saveConfig(config);
  }

  async updateStreamMappings(
    mappings: Array<Partial<StreamMapping>>,
  ): Promise<void> {
    const config = await this.getConfig();
    config.stream_mappings = normalizeStreamMappings(mappings);
    await this.saveConfig(config);
  }

  async updateSubdomainModeConfig(
    patch: Partial<SubdomainModeConfig>,
  ): Promise<SubdomainModeConfig> {
    const config = await this.getConfig();
    const next = normalizeSubdomainModeConfig({
      ...config.subdomain_mode,
      ...patch,
    });
    config.subdomain_mode = next;
    await this.saveConfig(config);
    return next;
  }

  async updateSSLConfig(ssl: SSLConfig): Promise<void> {
    await this.saveSSLCertificate({
      label: redisT("certificateLabels.current"),
      source: "manual",
      cert: ssl.cert,
      key: ssl.key,
      activate: true,
      matchBy: {
        cert: ssl.cert,
        key: ssl.key,
      },
    });
  }

  async addIPBackoff(ip: string, ttlSeconds: number): Promise<void> {
    await this.ephemeralStore.addIPBackoff(ip, ttlSeconds);
  }

  async getIPBackoff(ip: string): Promise<boolean> {
    return this.ephemeralStore.getIPBackoff(ip);
  }

  async addNonce(nonce: string, ttlSeconds: number = 300): Promise<void> {
    await this.ephemeralStore.addNonce(nonce, ttlSeconds);
  }

  async setNonceIfNotExists(
    nonce: string,
    ttlSeconds: number = 600,
  ): Promise<boolean> {
    return this.ephemeralStore.setNonceIfNotExists(nonce, ttlSeconds);
  }

  async setLockIfNotExists(
    lockName: string,
    ttlSeconds: number = 600,
  ): Promise<boolean> {
    return this.ephemeralStore.setLockIfNotExists(lockName, ttlSeconds);
  }

  async updateDefaultRoute(route: string): Promise<void> {
    const config = await this.getConfig();
    config.default_route = route;
    await this.saveConfig(config);
  }

  async updateDefaultTunnel(tunnel: "frp" | "cloudflared"): Promise<void> {
    const config = await this.getConfig();
    config.default_tunnel = tunnel;
    await this.saveConfig(config);
  }

  async getCAHosts(): Promise<string[]> {
    return this.caHostStore.getHosts();
  }

  async saveCAHosts(hosts: string[]): Promise<void> {
    await this.caHostStore.saveHosts(hosts);
  }

  async addCAHost(value: string): Promise<string[]> {
    return this.caHostStore.addHost(value);
  }

  async removeCAHost(value: string): Promise<string[]> {
    return this.caHostStore.removeHost(value);
  }

  async clearCAHosts(): Promise<void> {
    await this.caHostStore.clearHosts();
  }

  async getTOTPCredentials(): Promise<TOTPCredential[]> {
    return this.authCredentialStore.getTOTPCredentials();
  }

  async saveTOTPCredentials(totps: TOTPCredential[]): Promise<void> {
    await this.authCredentialStore.saveTOTPCredentials(totps);
  }

  async addTOTPCredential(totp: TOTPCredential): Promise<void> {
    await this.authCredentialStore.addTOTPCredential(totp);
  }

  async updateTOTPCredential(id: string, comment: string): Promise<boolean> {
    return this.authCredentialStore.updateTOTPCredential(id, comment);
  }

  async updateTOTPCredentialAccessScopes(
    id: string,
    accessScopes: unknown,
  ): Promise<TOTPCredential | null> {
    return this.authCredentialStore.updateTOTPCredentialAccessScopes(
      id,
      accessScopes,
    );
  }

  async updateTOTPCredentialSubdomainAccess(
    id: string,
    subdomainAccess: unknown,
  ): Promise<TOTPCredential | null> {
    return this.authCredentialStore.updateTOTPCredentialSubdomainAccess(
      id,
      subdomainAccess,
    );
  }

  async deleteTOTPCredential(id: string): Promise<boolean> {
    return this.authCredentialStore.deleteTOTPCredential(id);
  }

  async addSession(
    sessionId: string,
    session: LoginSession,
    ttlSeconds: number,
  ): Promise<void> {
    await this.authCredentialStore.addSession(sessionId, session, ttlSeconds);
  }

  async getSession(sessionId: string): Promise<LoginSession | null> {
    return this.authCredentialStore.getSession(sessionId);
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.authCredentialStore.deleteSession(sessionId);
  }

  async updateSession(
    sessionId: string,
    updates: Partial<LoginSession>,
  ): Promise<LoginSession | null> {
    return this.authCredentialStore.updateSession(sessionId, updates);
  }

  async isValidSession(sessionId: string): Promise<boolean> {
    return this.authCredentialStore.isValidSession(sessionId);
  }

  async listSessions(): Promise<Array<{ id: string; data: LoginSession }>> {
    return this.authCredentialStore.listSessions();
  }

  async getPasskeys(): Promise<PasskeyCredential[]> {
    return this.authCredentialStore.getPasskeys();
  }

  async savePasskeys(passkeys: PasskeyCredential[]): Promise<void> {
    await this.authCredentialStore.savePasskeys(passkeys);
  }

  async addPasskey(passkey: PasskeyCredential): Promise<void> {
    await this.authCredentialStore.addPasskey(passkey);
  }

  async deletePasskey(id: string): Promise<boolean> {
    return this.authCredentialStore.deletePasskey(id);
  }

  async updatePasskeyCounter(
    id: string,
    counter: number,
    lastUsedAt: string,
  ): Promise<boolean> {
    return this.authCredentialStore.updatePasskeyCounter(
      id,
      counter,
      lastUsedAt,
    );
  }

  async setPasskeyChallenge(
    challenge: string,
    type: "register" | "auth",
    ttlSeconds: number = 300,
  ): Promise<void> {
    await this.authCredentialStore.setPasskeyChallenge(
      challenge,
      type,
      ttlSeconds,
    );
  }

  async consumePasskeyChallenge(
    challenge: string,
    type: "register" | "auth",
  ): Promise<boolean> {
    return this.authCredentialStore.consumePasskeyChallenge(challenge, type);
  }

  async createPasskeyBindToken(
    totpId: string,
    ttlSeconds: number = 600,
  ): Promise<string> {
    return this.authCredentialStore.createPasskeyBindToken(totpId, ttlSeconds);
  }

  async isPasskeyBindTokenValid(token: string): Promise<boolean> {
    return this.authCredentialStore.isPasskeyBindTokenValid(token);
  }

  async consumePasskeyBindToken(token: string): Promise<string | null> {
    return this.authCredentialStore.consumePasskeyBindToken(token);
  }
}

export const configManager = new ConfigManager();
