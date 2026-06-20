import type Redis from "ioredis";
import { authMobilityKeys } from "./auth-mobility-keys";
import { AuthMobilityActiveIpStore } from "./auth-mobility-active-ip-store";
import { AuthMobilityBindingStore } from "./auth-mobility-binding-store";
import { AuthMobilityTimelineStore } from "./auth-mobility-timeline-store";
import { whitelistManager } from "./whitelist-manager";

export class AuthMobilitySessionCleanupService {
  constructor(
    private readonly redis: Redis,
    private readonly bindingStore: AuthMobilityBindingStore,
    private readonly activeIpStore: AuthMobilityActiveIpStore,
    private readonly timelineStore: AuthMobilityTimelineStore,
  ) {}

  async destroySession(sessionId: string): Promise<void> {
    const subjectKeys = await this.bindingStore.listSessionBindingKeys(
      sessionId,
    );
    const uniqueWhitelistRecordIds = new Set<string>();
    const proxyBinding = await this.bindingStore.get(
      "proxy-session",
      sessionId,
    );
    const activeIpDetails = await this.activeIpStore.listAllDetails(sessionId);

    if (proxyBinding?.whitelistRecordId) {
      uniqueWhitelistRecordIds.add(proxyBinding.whitelistRecordId);
    }

    for (const subjectKey of subjectKeys) {
      const binding = await this.bindingStore.getByStorageKey(subjectKey);
      if (binding?.whitelistRecordId) {
        uniqueWhitelistRecordIds.add(binding.whitelistRecordId);
      }
    }
    for (const detail of activeIpDetails) {
      if (detail.whitelistRecordId) {
        uniqueWhitelistRecordIds.add(detail.whitelistRecordId);
      }
    }

    const pipeline = this.redis.pipeline();
    this.bindingStore.queueClearBinding(pipeline, "proxy-session", sessionId);
    this.timelineStore.queueClearSession(pipeline, sessionId);
    if (subjectKeys.length > 0) {
      pipeline.del(...subjectKeys);
    }
    this.bindingStore.queueClearSessionIndex(pipeline, sessionId);
    for (const whitelistRecordId of uniqueWhitelistRecordIds) {
      pipeline.del(authMobilityKeys.whitelistOwner(whitelistRecordId));
    }
    await Promise.all([
      pipeline.exec(),
      this.activeIpStore.clearSession(sessionId),
    ]);

    for (const whitelistRecordId of uniqueWhitelistRecordIds) {
      await whitelistManager.removeWhiteList(whitelistRecordId);
    }
  }

  async getSessionWhitelistRecordId(
    sessionId: string,
  ): Promise<string | null> {
    const binding = await this.bindingStore.get("proxy-session", sessionId);
    return binding?.whitelistRecordId ?? null;
  }

  async listSessionWhitelistRecordIds(sessionId: string): Promise<string[]> {
    const recordIds = new Set<string>();
    const binding = await this.bindingStore.get("proxy-session", sessionId);
    if (binding?.whitelistRecordId) {
      recordIds.add(binding.whitelistRecordId);
    }
    for (const detail of await this.activeIpStore.listAllDetails(sessionId)) {
      if (detail.whitelistRecordId) {
        recordIds.add(detail.whitelistRecordId);
      }
    }
    return [...recordIds];
  }
}
