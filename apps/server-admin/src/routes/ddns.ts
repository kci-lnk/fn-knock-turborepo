import { Elysia, t } from "elysia";
import { ddnsLogBuffer, ddnsManager } from "../lib/ddns";
import { runAutomaticDDNSCheck } from "../lib/ddns/auto-check";
import { ddnsIntervalScheduler } from "../lib/ddns/scheduler";
import {
  DDNS_INTERFACE_IPV4_INDEX_FIELD,
  DDNS_INTERFACE_IPV6_INDEX_FIELD,
  DDNS_IP_SOURCE_FIELD,
  getDDNSTargetIPUnavailableMessage,
  resolveDDNSTargetIPs,
} from "../lib/ddns/ip-source";
import {
  applyUpdateScope,
  DDNS_UPDATE_SCOPE_FIELD,
  normalizeUpdateScope,
} from "../lib/ddns/providers/helpers";
import { DDNS_NETWORK_INTERFACE_FIELD } from "../lib/ddns/network";
import { emitDDNSUpdateCompletedEvent } from "../lib/system-events/helpers";
import { routeDoc, withRouteDoc } from "../lib/openapi";

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

const runTargetManualTest = async (targetId: string) => {
  const payload = await buildTargetPayload(targetId);
  if (!payload) {
    return {
      status: 404,
      body: { success: false, message: "未找到 DDNS 条目" },
    };
  }

  const { target, summary } = payload;
  if (!target.provider) {
    return {
      status: 400,
      body: { success: false, message: "请先选择 DDNS 提供商" },
    };
  }

  const complete = await ddnsManager.isTargetConfigComplete(target);
  if (!complete) {
    return {
      status: 400,
      body: {
        success: false,
        message: target.isPrimary
          ? "当前主域配置不完整，请填写所有必填字段"
          : "当前条目配置不完整，请填写所有必填字段",
      },
    };
  }

  try {
    await ddnsManager.appendTargetLog(
      "info",
      summary,
      "手动测试开始，正在解析当前目标 IP...",
    );

    await ddnsManager.ensureTargetAuxiliaryState(target, {
      emitLog: true,
      logPrefix: "手动测试",
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
    });

    await ddnsManager.appendTargetLog(
      "info",
      summary,
      `当前目标 IP（${ips.sourceLabel}） — IPv4: ${ips.ipv4 || "无"}, IPv6: ${ips.ipv6 || "无"}`,
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
        `${message}，测试中止`,
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
        `更新成功: ${result.message}`,
      );
    } else {
      await ddnsManager.setTargetLastCheck(target.id, "error", result.message);
      await ddnsManager.appendTargetLog(
        "error",
        summary,
        `更新失败: ${result.message}`,
      );
    }

    return {
      status: result.success ? 200 : 500,
      body: {
        success: result.success,
        message: result.message,
        data: { ipv4: ips.ipv4, ipv6: ips.ipv6 },
      },
    };
  } catch (error: any) {
    const message = error?.message || String(error);
    console.error("[ddns][manual-test] error:", error);
    await ddnsManager.setTargetLastCheck(target.id, "error", message);
    await ddnsManager.appendTargetLog("error", summary, `测试异常: ${message}`);
    return {
      status: 500,
      body: { success: false, message },
    };
  }
};

export const ddnsRoutes = new Elysia({
  prefix: "/api/admin/ddns",
  tags: ["DDNS"],
})
  .get(
    "/status",
    async () => {
      const status = await ddnsManager.getStatus();
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
    () => {
      return { success: true, data: ddnsManager.getProviders() };
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
    async ({ body, set }) => {
      try {
        const settings = await ddnsManager.updateSettings({
          updateIntervalMinutes: body.updateIntervalMinutes,
        });
        await ddnsIntervalScheduler.reload();
        return { success: true, data: settings };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || "保存 DDNS 自动同步设置失败",
        };
      }
    },
    withRouteDoc("更新 DDNS 自动同步设置", {
      body: t.Object({ updateIntervalMinutes: t.Number() }),
    }),
  )
  .get(
    "/interfaces",
    () => {
      return { success: true, data: ddnsManager.listNetworkInterfaces() };
    },
    routeDoc("获取可用网卡列表"),
  )
  .post(
    "/provider",
    async ({ body, set }) => {
      try {
        await ddnsManager.setProvider(body.provider);
        return { success: true };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || "设置提供商失败",
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
    async ({ params, body, set }) => {
      try {
        await ddnsManager.saveConfig(params.provider, body.config);
        return { success: true };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || "保存 DDNS 配置失败",
        };
      }
    },
    withRouteDoc("保存主域当前 DDNS 提供商配置", {
      body: t.Object({ config: t.Record(t.String(), t.String()) }),
    }),
  )
  .get(
    "/targets",
    async () => {
      return { success: true, data: await ddnsManager.getTargetsOverview() };
    },
    routeDoc("获取 DDNS 目标列表"),
  )
  .get(
    "/targets/:id",
    async ({ params, set }) => {
      const payload = await buildTargetPayload(params.id);
      if (!payload) {
        set.status = 404;
        return { success: false, message: "未找到 DDNS 条目" };
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
    async ({ body, set }) => {
      try {
        const target = await ddnsManager.createTarget({
          name: body.name,
          provider: body.provider,
          enabled: body.enabled,
          config: body.config,
        });
        const summary = await ddnsManager.buildTargetSummary(target.id);
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
          message: error?.message || "创建 DDNS 条目失败",
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
    async ({ params, body, set }) => {
      try {
        const target = await ddnsManager.updateTarget(params.id, {
          name: body.name,
          provider: body.provider,
          enabled: body.enabled,
          config: body.config,
        });
        const summary = await ddnsManager.buildTargetSummary(target.id);
        return {
          success: true,
          data: {
            ...(summary || {}),
            rawName: target.name,
            config: target.config,
          },
        };
      } catch (error: any) {
        const message = error?.message || "更新 DDNS 条目失败";
        set.status = message.includes("未找到") ? 404 : 400;
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
    async ({ params, set }) => {
      try {
        await ddnsManager.deleteTarget(params.id);
        return { success: true };
      } catch (error: any) {
        const message = error?.message || "删除 DDNS 条目失败";
        set.status = message.includes("未找到") ? 404 : 400;
        return { success: false, message };
      }
    },
    routeDoc("删除 DDNS 条目"),
  )
  .post(
    "/targets/:id/enabled",
    async ({ params, body, set }) => {
      try {
        await ddnsManager.setTargetEnabled(params.id, body.enabled);
        return { success: true };
      } catch (error: any) {
        const message = error?.message || "更新 DDNS 条目启用状态失败";
        set.status = message.includes("未找到") ? 404 : 400;
        return { success: false, message };
      }
    },
    withRouteDoc("更新 DDNS 条目启用状态", {
      body: t.Object({ enabled: t.Boolean() }),
    }),
  )
  .post(
    "/test",
    async ({ set }) => {
      const primaryTarget = await ddnsManager.getPrimaryTarget();
      const result = await runTargetManualTest(primaryTarget.id);
      set.status = result.status;
      return result.body;
    },
    routeDoc("手动触发主域 DDNS 测试更新"),
  )
  .post(
    "/targets/:id/test",
    async ({ params, set }) => {
      const result = await runTargetManualTest(params.id);
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
