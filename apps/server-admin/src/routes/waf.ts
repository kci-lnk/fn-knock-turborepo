import { Elysia, t } from "elysia";
import { goBackend } from "../lib/go-backend";
import {
  applyWAFConfig,
  deleteCustomWAFRule,
  drainWAFEventsNow,
  getWAFDetails,
  readWAFRuleFile,
  refreshSystemManifestCache,
  setWAFRuleEnabled,
  syncSystemWAFRules,
  uploadCustomWAFRules,
} from "../lib/waf/service";
import { wafLogStore } from "../lib/waf/log-store";
import { routeDoc, withRouteDoc } from "../lib/openapi";

const toFailure = (
  set: { status?: number | string },
  message: string,
  status = 400,
) => {
  set.status = status;
  return {
    success: false,
    message,
  };
};

export const wafRoutes = new Elysia({
  prefix: "/api/admin/waf",
  tags: ["WAF"],
})
  .get(
    "/details",
    async () => ({
      success: true,
      data: await getWAFDetails(),
    }),
    routeDoc("获取 WAF 设置、系统规则、自定义规则与网关状态"),
  )
  .get(
    "/status",
    async ({ set }) => {
      const response = await goBackend.getWAFStatus();
      if (!response.success || !response.data) {
        return toFailure(set, response.message || "读取 WAF 状态失败", 502);
      }
      return { success: true, data: response.data };
    },
    routeDoc("获取 Go 网关 WAF 状态"),
  )
  .post(
    "/config",
    async ({ body, set }) => {
      try {
        return {
          success: true,
          data: await applyWAFConfig(body as any),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "WAF 设置保存或加载失败", 400);
      }
    },
    withRouteDoc("保存 WAF 开关、防护强度、规则自动更新并按需加载当前规则", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        system_rules_auto_update_enabled: t.Optional(t.Boolean()),
        common_location_exempt_enabled: t.Optional(t.Boolean()),
        paranoia_level: t.Optional(t.Number()),
        executing_paranoia_level: t.Optional(t.Number()),
      }),
    }),
  )
  .post(
    "/manifest/refresh",
    async ({ set }) => {
      try {
        await refreshSystemManifestCache();
        return {
          success: true,
          data: await getWAFDetails(),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "系统规则清单刷新失败", 502);
      }
    },
    routeDoc("刷新系统 WAF 规则清单"),
  )
  .post(
    "/system/sync",
    async ({ set }) => {
      try {
        return {
          success: true,
          data: await syncSystemWAFRules(),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "系统规则同步失败", 502);
      }
    },
    routeDoc("下载并同步系统 WAF 规则"),
  )
  .post(
    "/rules/enabled",
    async ({ body, set }) => {
      try {
        return {
          success: true,
          data: await setWAFRuleEnabled(body as any),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "WAF 规则启停失败", 400);
      }
    },
    withRouteDoc("批量启用或停用 WAF 规则文件", {
      body: t.Object({
        source: t.Union([t.Literal("system"), t.Literal("custom")]),
        filenames: t.Optional(t.Array(t.String())),
        enabled: t.Boolean(),
      }),
    }),
  )
  .get(
    "/rules/:source/:filename",
    async ({ params, set }) => {
      try {
        return {
          success: true,
          data: await readWAFRuleFile(
            params.source as "system" | "custom",
            params.filename,
          ),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "读取 WAF 规则失败", 404);
      }
    },
    withRouteDoc("读取 WAF 规则文件内容", {
      params: t.Object({
        source: t.Union([t.Literal("system"), t.Literal("custom")]),
        filename: t.String(),
      }),
    }),
  )
  .post(
    "/custom/upload",
    async ({ body, set }) => {
      try {
        return {
          success: true,
          data: await uploadCustomWAFRules(body as any),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "自定义规则上传失败", 400);
      }
    },
    withRouteDoc("上传自定义 WAF 规则文件", {
      body: t.Object({
        files: t.Array(
          t.Object({
            filename: t.String(),
            content_base64: t.String(),
          }),
        ),
      }),
    }),
  )
  .delete(
    "/custom/:filename",
    async ({ params, set }) => {
      try {
        return {
          success: true,
          data: await deleteCustomWAFRule(params.filename),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "自定义规则删除失败", 400);
      }
    },
    withRouteDoc("删除自定义 WAF 规则文件", {
      params: t.Object({ filename: t.String() }),
    }),
  )
  .post(
    "/events/drain",
    async ({ set }) => {
      try {
        return {
          success: true,
          data: await drainWAFEventsNow(),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "拉取 WAF 事件失败", 502);
      }
    },
    routeDoc("立即拉取并持久化 Go WAF 事件"),
  )
  .get(
    "/logs",
    async ({ query, set }) => {
      try {
        return {
          success: true,
          data: await wafLogStore.query(query),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "查询 WAF 日志失败", 400);
      }
    },
    withRouteDoc("查询 WAF 日志", {
      query: t.Object({
        date: t.Optional(t.String()),
        trace_id: t.Optional(t.String()),
        search: t.Optional(t.String()),
        host: t.Optional(t.String()),
        client_ip: t.Optional(t.String()),
        rule_id: t.Optional(t.String()),
        route_type: t.Optional(t.String()),
        mode: t.Optional(t.String()),
        cursor: t.Optional(t.String()),
        limit: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/logs/:traceId",
    async ({ params, set }) => {
      const event = await wafLogStore.getEvent(params.traceId);
      if (!event) return toFailure(set, "WAF 日志不存在", 404);
      return { success: true, data: event };
    },
    withRouteDoc("按 trace ID 获取 WAF 日志详情", {
      params: t.Object({
        traceId: t.String(),
      }),
    }),
  )
  .delete(
    "/logs",
    async ({ body, set }) => {
      try {
        return {
          success: true,
          data: await wafLogStore.deleteDate(body.date),
        };
      } catch (error: any) {
        return toFailure(set, error?.message || "删除 WAF 日志失败", 400);
      }
    },
    withRouteDoc("按日期删除 WAF 日志", {
      body: t.Object({
        date: t.String(),
      }),
    }),
  );
