import { Elysia, t } from "elysia";
import { ddnsLogBuffer, ddnsManager } from "../lib/ddns";
import { runAutomaticDDNSCheck } from "../lib/ddns/auto-check";
import { ddnsIntervalScheduler } from "../lib/ddns/scheduler";
import {
  DDNS_INTERFACE_IPV4_INDEX_FIELD,
  DDNS_INTERFACE_IPV6_INDEX_FIELD,
  DDNS_IP_SOURCE_FIELD,
  DDNS_SOURCE_DOMAIN_FIELD,
  DDNS_STATIC_IPV4_FIELD,
  DDNS_STATIC_IPV6_FIELD,
  getDDNSTargetIPUnavailableMessage,
  resolveDDNSTargetIPs,
} from "../lib/ddns/ip-source";
import {
  applyUpdateScope,
  DDNS_UPDATE_SCOPE_FIELD,
  normalizeUpdateScope,
  withDDNSLocale,
} from "../lib/ddns/providers/helpers";
import { DDNS_NETWORK_INTERFACE_FIELD } from "../lib/ddns/network";
import { emitDDNSUpdateCompletedEvent } from "../lib/system-events/helpers";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator, tDefault } from "../lib/i18n";
import { IPDetector } from "../plugins/ip-detector";

const getDDNSRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

type DDNSRouteTranslator = ReturnType<typeof createRequestTranslator>["t"];
const isTargetNotFoundMessage = (message: string, t: DDNSRouteTranslator) =>
  message === t("server.ddns.targetNotFound") ||
  message === tDefault("server.ddns.targetNotFound");

const parseDDNSLogEntries = (raw: string[]) =>
  raw.map((line) => {
    try {
      return JSON.parse(line);
    } catch {
      return { time: "", level: "info", message: line };
    }
  });

const buildTargetPayload = async (targetId: string) => {
  const target = await ddnsManager.getTarget(targetId);
  const summary = await ddnsManager.buildTargetSummary(targetId);
  if (!target || !summary) {
    return null;
  }

  return {
    target,
    summary,
  };
};

const runTargetManualTest = async (
  targetId: string,
  t: DDNSRouteTranslator,
  locale: string,
) => {
  return withDDNSLocale(locale, async () => {
    const payload = await buildTargetPayload(targetId);
    if (!payload) {
      return {
        status: 404,
        body: { success: false, message: t("server.ddns.targetNotFound") },
      };
    }

    const { target, summary } = payload;
    if (!target.provider) {
      return {
        status: 400,
        body: { success: false, message: t("server.ddns.selectProviderFirst") },
      };
    }

    const complete = await ddnsManager.isTargetConfigComplete(target);
    if (!complete) {
      return {
        status: 400,
        body: {
          success: false,
          message: target.isPrimary
            ? t("server.ddns.primaryConfigIncomplete")
            : t("server.ddns.targetConfigIncomplete"),
        },
      };
    }

    try {
      await ddnsManager.appendTargetLog(
        "info",
        summary,
        t("server.ddns.manualTestStart"),
      );

      await ddnsManager.ensureTargetAuxiliaryState(target, {
        emitLog: true,
        logPrefix: t("server.ddns.manualTestPrefix"),
      });

      const updateScope = normalizeUpdateScope(
        target.config[DDNS_UPDATE_SCOPE_FIELD],
      );
      const settings = await ddnsManager.getSettings();
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

      await ddnsManager.appendTargetLog(
        "info",
        summary,
        t("server.ddns.currentTargetIp", {
          source: ips.sourceLabel,
          ipv4: ips.ipv4 || t("server.ddns.none"),
          ipv6: ips.ipv6 || t("server.ddns.none"),
        }),
      );
      for (const warning of ips.warnings) {
        await ddnsManager.appendTargetLog("warn", summary, warning);
      }

      const scopedIPs = applyUpdateScope(updateScope, ips.ipv4, ips.ipv6);
      if (!scopedIPs.ipv4 && !scopedIPs.ipv6) {
        const message = getDDNSTargetIPUnavailableMessage(
          ips.source,
          updateScope,
        );
        await ddnsManager.setTargetLastCheck(target.id, "error", message);
        await ddnsManager.appendTargetLog(
          "error",
          summary,
          t("server.ddns.testAborted", { message }),
        );
        return {
          status: 500,
          body: { success: false, message },
        };
      }

      const previousIp = await ddnsManager.getTargetLastIP(target.id);
      const result = await ddnsManager.executeTargetUpdate(
        target,
        ips.ipv4,
        ips.ipv6,
        locale,
      );

      await emitDDNSUpdateCompletedEvent({
        trigger: "manual_test",
        targetId: target.id,
        targetName: summary.name,
        domainSummary: summary.domainSummary,
        isPrimary: target.isPrimary,
        provider: target.provider,
        success: result.success,
        message: result.message,
        updateScope,
        ipSource: ips.source,
        previousIpv4: previousIp.ipv4,
        previousIpv6: previousIp.ipv6,
        nextIpv4: scopedIPs.ipv4,
        nextIpv6: scopedIPs.ipv6,
      });

      if (result.success) {
        await ddnsManager.setTargetLastIP(
          target.id,
          scopedIPs.ipv4,
          scopedIPs.ipv6,
          {
            merge: true,
          },
        );
        await ddnsManager.setTargetLastCheck(
          target.id,
          "updated",
          result.message,
        );
        await ddnsManager.appendTargetLog(
          "info",
          summary,
          t("server.ddns.updateSuccess", { message: result.message }),
        );
      } else {
        await ddnsManager.setTargetLastCheck(
          target.id,
          "error",
          result.message,
        );
        await ddnsManager.appendTargetLog(
          "error",
          summary,
          t("server.ddns.updateFailed", { message: result.message }),
        );
      }

      return {
        status: result.success ? 200 : 500,
        body: {
          success: result.success,
          message: result.message,
          data: {
            ipv4: ips.ipv4,
            ipv6: ips.ipv6,
            source: ips.source,
            sourceLabel: ips.sourceLabel,
          },
        },
      };
    } catch (error: any) {
      const message = error?.message || String(error);
      console.error("[ddns][manual-test] error:", error);
      await ddnsManager.setTargetLastCheck(target.id, "error", message);
      await ddnsManager.appendTargetLog(
        "error",
        summary,
        t("server.ddns.testError", { message }),
      );
      return {
        status: 500,
        body: { success: false, message },
      };
    }
  });
};

export const ddnsRoutes = new Elysia({
  prefix: "/api/admin/ddns",
  tags: ["DDNS"],
})
  .get(
    "/status",
    async ({ request }) => {
      const { locale } = await getDDNSRouteTranslator(request);
      const status = await withDDNSLocale(locale, () =>
        ddnsManager.getStatus(),
      );
      return { success: true, data: status };
    },
    routeDoc("获取 DDNS 当前状态"),
  )
  .post(
    "/toggle",
    async ({ body }) => {
      const wasEnabled = await ddnsManager.isEnabled();
      await ddnsManager.setEnabled(body.enabled);

      if (body.enabled && !wasEnabled) {
        void runAutomaticDDNSCheck({
          trigger: "enable",
          emitSkipLog: true,
        });
      }

      return { success: true };
    },
    withRouteDoc("启用或停用 DDNS", {
      body: t.Object({ enabled: t.Boolean() }),
    }),
  )
  .get(
    "/providers",
    async ({ request }) => {
      const { locale } = await getDDNSRouteTranslator(request);
      return { success: true, data: ddnsManager.getProviders(locale) };
    },
    routeDoc("获取 DDNS 提供商列表"),
  )
  .get(
    "/settings",
    async () => {
      return { success: true, data: await ddnsManager.getSettings() };
    },
    routeDoc("获取 DDNS 自动同步设置"),
  )
  .post(
    "/settings",
    async ({ body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        const settings = await withDDNSLocale(locale, () =>
          ddnsManager.updateSettings({
            updateIntervalMinutes: body.updateIntervalMinutes,
            publicCheckSources: body.publicCheckSources,
            httpTransport: body.httpTransport,
          }),
        );
        await ddnsIntervalScheduler.reload();
        return { success: true, data: settings };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.ddns.settingsSaveFailed"),
        };
      }
    },
    withRouteDoc("更新 DDNS 自动同步设置", {
      body: t.Object({
        updateIntervalMinutes: t.Optional(t.Number()),
        publicCheckSources: t.Optional(
          t.Object({
            ipv4: t.Array(t.String()),
            ipv6: t.Array(t.String()),
          }),
        ),
        httpTransport: t.Optional(t.Union([t.Literal("curl"), t.Literal("node")])),
      }),
    }),
  )
  .post(
    "/public-check/test",
    async ({ body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        const settings = await ddnsManager.getSettings();
        const results = await withDDNSLocale(locale, () =>
          IPDetector.testPublicCheckSources(body.publicCheckSources, {
            httpTransport: body.httpTransport ?? settings.httpTransport,
            networkInterface: body.networkInterface,
          }),
        );
        return { success: true, data: { results } };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.ddns.publicCheckTestFailed"),
        };
      }
    },
    withRouteDoc("测试 DDNS 公网探测地址", {
      body: t.Object({
        publicCheckSources: t.Object({
          ipv4: t.Array(t.String()),
          ipv6: t.Array(t.String()),
        }),
        httpTransport: t.Optional(t.Union([t.Literal("curl"), t.Literal("node")])),
        networkInterface: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/interfaces",
    async ({ request }) => {
      const { locale } = await getDDNSRouteTranslator(request);
      return {
        success: true,
        data: withDDNSLocale(locale, () => ddnsManager.listNetworkInterfaces()),
      };
    },
    routeDoc("获取可用网卡列表"),
  )
  .post(
    "/provider",
    async ({ body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        await withDDNSLocale(locale, () =>
          ddnsManager.setProvider(body.provider),
        );
        return { success: true };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.ddns.providerSetFailed"),
        };
      }
    },
    withRouteDoc("设置主域 DDNS 提供商", {
      body: t.Object({ provider: t.String() }),
    }),
  )
  .get(
    "/config/:provider",
    async ({ params }) => {
      const config = await ddnsManager.getConfig(params.provider);
      return { success: true, data: config };
    },
    routeDoc("获取主域当前 DDNS 提供商配置"),
  )
  .post(
    "/config/:provider",
    async ({ params, body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        await withDDNSLocale(locale, () =>
          ddnsManager.saveConfig(params.provider, body.config),
        );
        return { success: true };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.ddns.configSaveFailed"),
        };
      }
    },
    withRouteDoc("保存主域当前 DDNS 提供商配置", {
      body: t.Object({ config: t.Record(t.String(), t.String()) }),
    }),
  )
  .get(
    "/targets",
    async ({ request }) => {
      const { locale } = await getDDNSRouteTranslator(request);
      return {
        success: true,
        data: await withDDNSLocale(locale, () =>
          ddnsManager.getTargetsOverview(),
        ),
      };
    },
    routeDoc("获取 DDNS 目标列表"),
  )
  .get(
    "/targets/:id",
    async ({ params, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      const payload = await withDDNSLocale(locale, () =>
        buildTargetPayload(params.id),
      );
      if (!payload) {
        set.status = 404;
        return { success: false, message: t("server.ddns.targetNotFound") };
      }

      return {
        success: true,
        data: {
          ...payload.summary,
          rawName: payload.target.name,
          config: payload.target.config,
        },
      };
    },
    routeDoc("获取单个 DDNS 目标详情"),
  )
  .post(
    "/targets",
    async ({ body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        const { target, summary } = await withDDNSLocale(locale, async () => {
          const target = await ddnsManager.createTarget({
            name: body.name,
            provider: body.provider,
            enabled: body.enabled,
            config: body.config,
          });
          const summary = await ddnsManager.buildTargetSummary(target.id);
          return { target, summary };
        });
        return {
          success: true,
          data: {
            ...(summary || {}),
            rawName: target.name,
            config: target.config,
          },
        };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.ddns.createTargetFailed"),
        };
      }
    },
    withRouteDoc("创建 DDNS 条目", {
      body: t.Object({
        name: t.Optional(t.String()),
        provider: t.String(),
        enabled: t.Optional(t.Boolean()),
        config: t.Record(t.String(), t.String()),
      }),
    }),
  )
  .put(
    "/targets/:id",
    async ({ params, body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        const { target, summary } = await withDDNSLocale(locale, async () => {
          const target = await ddnsManager.updateTarget(params.id, {
            name: body.name,
            provider: body.provider,
            enabled: body.enabled,
            config: body.config,
          });
          const summary = await ddnsManager.buildTargetSummary(target.id);
          return { target, summary };
        });
        return {
          success: true,
          data: {
            ...(summary || {}),
            rawName: target.name,
            config: target.config,
          },
        };
      } catch (error: any) {
        const message = error?.message || t("server.ddns.updateTargetFailed");
        set.status = isTargetNotFoundMessage(message, t) ? 404 : 400;
        return { success: false, message };
      }
    },
    withRouteDoc("更新 DDNS 条目", {
      body: t.Object({
        name: t.Optional(t.String()),
        provider: t.String(),
        enabled: t.Optional(t.Boolean()),
        config: t.Record(t.String(), t.String()),
      }),
    }),
  )
  .delete(
    "/targets/:id",
    async ({ params, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        await withDDNSLocale(locale, () => ddnsManager.deleteTarget(params.id));
        return { success: true };
      } catch (error: any) {
        const message = error?.message || t("server.ddns.deleteTargetFailed");
        set.status = isTargetNotFoundMessage(message, t) ? 404 : 400;
        return { success: false, message };
      }
    },
    routeDoc("删除 DDNS 条目"),
  )
  .post(
    "/targets/:id/enabled",
    async ({ params, body, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      try {
        await withDDNSLocale(locale, () =>
          ddnsManager.setTargetEnabled(params.id, body.enabled),
        );
        return { success: true };
      } catch (error: any) {
        const message =
          error?.message || t("server.ddns.updateTargetEnabledFailed");
        set.status = isTargetNotFoundMessage(message, t) ? 404 : 400;
        return { success: false, message };
      }
    },
    withRouteDoc("更新 DDNS 条目启用状态", {
      body: t.Object({ enabled: t.Boolean() }),
    }),
  )
  .post(
    "/test",
    async ({ set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      const primaryTarget = await ddnsManager.getPrimaryTarget();
      const result = await runTargetManualTest(primaryTarget.id, t, locale);
      set.status = result.status;
      return result.body;
    },
    routeDoc("手动触发主域 DDNS 测试更新"),
  )
  .post(
    "/targets/:id/test",
    async ({ params, set, request }) => {
      const { locale, t } = await getDDNSRouteTranslator(request);
      const result = await runTargetManualTest(params.id, t, locale);
      set.status = result.status;
      return result.body;
    },
    routeDoc("手动触发单个 DDNS 条目测试更新"),
  )
  .get(
    "/logs",
    async ({ query }) => {
      const limit = Math.max(
        1,
        Math.min(parseInt((query.limit as any) || "200", 10), 1000),
      );
      const logs = await ddnsManager.getLogs(limit);
      return { success: true, data: logs };
    },
    routeDoc("获取 DDNS 日志"),
  )
  .delete(
    "/logs",
    async () => {
      await ddnsManager.clearLogs();
      return { success: true };
    },
    routeDoc("清空 DDNS 日志"),
  )
  .get(
    "/poll",
    async ({ query }) => {
      const { cursor, reset, items } = await ddnsLogBuffer.poll(query.cursor);
      const status = await ddnsManager.getStatus();

      return {
        success: true,
        data: {
          cursor,
          reset,
          logs: parseDDNSLogEntries(items),
          status,
        },
      };
    },
    withRouteDoc("轮询 DDNS 日志与状态", {
      query: t.Object({
        cursor: t.Optional(t.String()),
      }),
    }),
  );
