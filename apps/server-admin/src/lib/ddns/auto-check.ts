import { randomUUID } from "node:crypto";
import { configManager, redis } from "../redis";
import { ddnsManager } from ".";
import {
  DDNS_INTERFACE_IPV4_INDEX_FIELD,
  DDNS_INTERFACE_IPV6_INDEX_FIELD,
  DDNS_IP_SOURCE_FIELD,
  DDNS_SOURCE_DOMAIN_FIELD,
  DDNS_STATIC_IPV4_FIELD,
  DDNS_STATIC_IPV6_FIELD,
  getDDNSTargetIPUnavailableMessage,
  resolveDDNSTargetIPs,
} from "./ip-source";
import {
  applyUpdateScope,
  ddnsTranslate,
  DDNS_UPDATE_SCOPE_FIELD,
  normalizeUpdateScope,
  withDDNSLocale,
} from "./providers/helpers";
import { DDNS_NETWORK_INTERFACE_FIELD } from "./network";
import { emitDDNSUpdateCompletedEvent } from "../system-events/helpers";

const DDNS_UPDATE_LOCK_NAME = "ddns-update";
const DDNS_UPDATE_LOCK_TTL_SECONDS = 600;
const DDNS_UPDATE_LOCK_KEY = `fn_knock:lock:${DDNS_UPDATE_LOCK_NAME}`;
const RELEASE_LOCK_SCRIPT = `
if redis.call("GET", KEYS[1]) == ARGV[1] then
  return redis.call("DEL", KEYS[1])
end
return 0
`;
const REFRESH_LOCK_SCRIPT = `
if redis.call("GET", KEYS[1]) == ARGV[1] then
  return redis.call("EXPIRE", KEYS[1], ARGV[2])
end
return 0
`;

export type DDNSAutoCheckTrigger = "cron" | "enable" | "startup";

export type RunAutomaticDDNSCheckOptions = {
  trigger?: DDNSAutoCheckTrigger;
  emitSkipLog?: boolean;
  emitNoopLog?: boolean;
};

const TRIGGER_LABEL_KEYS: Record<DDNSAutoCheckTrigger, string> = {
  cron: "triggerCron",
  enable: "triggerEnable",
  startup: "triggerStartup",
};

const ddnsT = ddnsTranslate;

const withTrigger = (trigger: string, message: string): string =>
  ddnsT("triggerMessage", { trigger, message });

const recordSkippedCheck = async (
  targetId: string,
  message: string,
  emitLog: boolean,
) => {
  await ddnsManager.setTargetLastCheck(targetId, "skipped", message);
  if (!emitLog) {
    return;
  }

  const summary = await ddnsManager.buildTargetSummary(targetId);
  if (summary) {
    await ddnsManager.appendTargetLog("warn", summary, message);
  } else {
    await ddnsManager.appendLog("warn", message);
  }
};

const acquireDDNSLock = async (token: string): Promise<boolean> => {
  const result = await redis.set(
    DDNS_UPDATE_LOCK_KEY,
    token,
    "EX",
    DDNS_UPDATE_LOCK_TTL_SECONDS,
    "NX",
  );
  return result === "OK";
};

const refreshDDNSLock = async (token: string): Promise<void> => {
  await (redis as any).eval(
    REFRESH_LOCK_SCRIPT,
    1,
    DDNS_UPDATE_LOCK_KEY,
    token,
    String(DDNS_UPDATE_LOCK_TTL_SECONDS),
  );
};

const releaseDDNSLock = async (token: string): Promise<void> => {
  await (redis as any).eval(
    RELEASE_LOCK_SCRIPT,
    1,
    DDNS_UPDATE_LOCK_KEY,
    token,
  );
};

const runAutomaticDDNSCheckWithLocale = async (
  options: RunAutomaticDDNSCheckOptions = {},
) => {
  const trigger = options.trigger ?? "cron";
  const triggerLabel = ddnsT(TRIGGER_LABEL_KEYS[trigger]);
  const lockToken = randomUUID();

  const enabled = await ddnsManager.isEnabled();
  if (!enabled) {
    return;
  }

  const acquired = await acquireDDNSLock(lockToken);
  if (!acquired) {
    return;
  }

  try {
    const [targets, settings] = await Promise.all([
      ddnsManager.listRunnableTargets(),
      ddnsManager.getSettings(),
    ]);

    for (const target of targets) {
      const summary = (await ddnsManager.buildTargetSummary(target.id)) || {
        id: target.id,
        name: target.name,
        isPrimary: target.isPrimary,
        enabled: target.enabled,
        provider: target.provider,
        updateScope: normalizeUpdateScope(
          target.config[DDNS_UPDATE_SCOPE_FIELD],
        ),
        providerLabel: target.provider || ddnsT("notConfigured"),
        domainSummary: "",
        createdAt: target.createdAt,
        updatedAt: target.updatedAt,
        sortOrder: target.sortOrder,
        lastIP: target.lastIP,
        lastCheck: target.lastCheck,
      };

      try {
        if (!target.provider) {
          await recordSkippedCheck(
            target.id,
            withTrigger(triggerLabel, ddnsT("skippedNoProvider")),
            options.emitSkipLog === true,
          );
          continue;
        }

        const completeness =
          await ddnsManager.getTargetConfigCompleteness(target);
        if (!completeness.complete) {
          const message = completeness.reason
            ? `${ddnsT("skippedIncompleteConfig")}: ${completeness.reason}`
            : ddnsT("skippedIncompleteConfig");
          await recordSkippedCheck(
            target.id,
            withTrigger(triggerLabel, message),
            options.emitSkipLog === true,
          );
          continue;
        }

        await ddnsManager.ensureTargetAuxiliaryState(target, {
          emitLog: true,
          logPrefix: triggerLabel,
        });

        const updateScope = normalizeUpdateScope(
          target.config[DDNS_UPDATE_SCOPE_FIELD],
        );
        const ips = await resolveDDNSTargetIPs({
          updateScope,
          ipSource: target.config[DDNS_IP_SOURCE_FIELD],
          networkInterface: target.config[DDNS_NETWORK_INTERFACE_FIELD],
          interfaceIpv4Index: target.config[DDNS_INTERFACE_IPV4_INDEX_FIELD],
          interfaceIpv6Index: target.config[DDNS_INTERFACE_IPV6_INDEX_FIELD],
          staticIpv4: target.config[DDNS_STATIC_IPV4_FIELD],
          staticIpv6: target.config[DDNS_STATIC_IPV6_FIELD],
          sourceDomain: target.config[DDNS_SOURCE_DOMAIN_FIELD],
          publicCheckSources: settings.publicCheckSources,
          httpTransport: settings.httpTransport,
        });

        for (const warning of ips.warnings) {
          await ddnsManager.appendTargetLog(
            "warn",
            summary,
            withTrigger(triggerLabel, warning),
          );
        }

        if (ips.source === "public" && !ips.ipv4 && !ips.ipv6) {
          const message = withTrigger(
            triggerLabel,
            ddnsT("skippedPublicIpUnavailable"),
          );
          await ddnsManager.setTargetLastCheck(target.id, "error", message);
          await ddnsManager.appendTargetLog("error", summary, message);
          continue;
        }

        const scopedIPs = applyUpdateScope(updateScope, ips.ipv4, ips.ipv6);
        if (!scopedIPs.ipv4 && !scopedIPs.ipv6) {
          const message = withTrigger(
            triggerLabel,
            ddnsT("skippedReason", {
              reason: getDDNSTargetIPUnavailableMessage(
                ips.source,
                updateScope,
              ),
            }),
          );
          await ddnsManager.setTargetLastCheck(target.id, "skipped", message);
          await ddnsManager.appendTargetLog("warn", summary, message);
          continue;
        }

        const lastIP = await ddnsManager.getTargetLastIP(target.id);
        const ipv4Changed = !!scopedIPs.ipv4 && scopedIPs.ipv4 !== lastIP.ipv4;
        const ipv6Changed = !!scopedIPs.ipv6 && scopedIPs.ipv6 !== lastIP.ipv6;

        if (!ipv4Changed && !ipv6Changed) {
          const message = withTrigger(triggerLabel, ddnsT("targetIpNoChange"));
          await ddnsManager.setTargetLastCheck(target.id, "noop", message);
          if (options.emitNoopLog === true) {
            await ddnsManager.appendTargetLog("info", summary, message);
          }
          continue;
        }

        const changes: string[] = [];
        if (ipv4Changed) {
          changes.push(
            ddnsT("ipChange", {
              family: "IPv4",
              before: lastIP.ipv4 || ddnsT("none"),
              after: scopedIPs.ipv4 || ddnsT("none"),
            }),
          );
        }
        if (ipv6Changed) {
          changes.push(
            ddnsT("ipChange", {
              family: "IPv6",
              before: lastIP.ipv6 || ddnsT("none"),
              after: scopedIPs.ipv6 || ddnsT("none"),
            }),
          );
        }
        await ddnsManager.appendTargetLog(
          "info",
          summary,
          withTrigger(
            triggerLabel,
            ddnsT("targetIpChanged", { changes: changes.join(", ") }),
          ),
        );

        const result = await ddnsManager.executeTargetUpdate(
          target,
          ips.ipv4,
          ips.ipv6,
        );
        await emitDDNSUpdateCompletedEvent({
          trigger,
          targetId: target.id,
          targetName: summary.name,
          domainSummary: summary.domainSummary,
          isPrimary: target.isPrimary,
          provider: target.provider,
          success: result.success,
          message: result.message,
          updateScope,
          ipSource: ips.source,
          previousIpv4: lastIP.ipv4,
          previousIpv6: lastIP.ipv6,
          nextIpv4: scopedIPs.ipv4,
          nextIpv6: scopedIPs.ipv6,
        });

        if (result.success) {
          const message = withTrigger(
            triggerLabel,
            ddnsT("dnsUpdateSuccess", {
              provider: target.provider,
              message: result.message,
            }),
          );
          await ddnsManager.setTargetLastIP(
            target.id,
            scopedIPs.ipv4,
            scopedIPs.ipv6,
            {
              merge: true,
            },
          );
          await ddnsManager.setTargetLastCheck(target.id, "updated", message);
          await ddnsManager.appendTargetLog("info", summary, message);
          continue;
        }

        const message = withTrigger(
          triggerLabel,
          ddnsT("dnsUpdateFailed", {
            provider: target.provider,
            message: result.message,
          }),
        );
        await ddnsManager.setTargetLastCheck(target.id, "error", message);
        await ddnsManager.appendTargetLog("error", summary, message);
      } catch (error: any) {
        const message = withTrigger(
          triggerLabel,
          ddnsT("taskError", { message: error?.message || String(error) }),
        );
        console.error("[ddns][auto-check] error:", error);
        await ddnsManager.setTargetLastCheck(target.id, "error", message);
        await ddnsManager.appendTargetLog("error", summary, message);
      } finally {
        await refreshDDNSLock(lockToken).catch(() => undefined);
      }
    }
  } finally {
    await releaseDDNSLock(lockToken).catch(() => undefined);
  }
};

export const runAutomaticDDNSCheck = async (
  options: RunAutomaticDDNSCheckOptions = {},
) => {
  const localeConfig = await configManager.getLocaleConfig();
  return withDDNSLocale(localeConfig.default_locale, () =>
    runAutomaticDDNSCheckWithLocale(options),
  );
};
