import { Elysia, t } from "elysia";
import { goBackend } from "../lib/go-backend";
import {
  getGatewayLoggingConfigForResponse,
  syncGatewayLoggingToGateway,
} from "../lib/gateway-logging";
import { configManager } from "../lib/redis";

const toFailure = (
  set: { status?: number | string },
  message: string,
  status = 502,
) => {
  set.status = status;
  return {
    success: false,
    message,
  };
};

export const gatewayLogsRoutes = new Elysia({
  prefix: "/api/admin/gateway-logs",
})
  .get("/config", async () => {
    const settings = await configManager.getGatewayLoggingConfig();
    return {
      success: true,
      data: await getGatewayLoggingConfigForResponse(settings),
    };
  })
  .post(
    "/config",
    async ({ body, set }) => {
      const settings = await configManager.updateGatewayLoggingConfig({
        enabled: body.enabled,
        max_days: body.max_days,
      });

      try {
        const data = await syncGatewayLoggingToGateway(settings);
        return { success: true, data };
      } catch (error: any) {
        return toFailure(
          set,
          error?.message || "请求日志设置已保存，但同步到网关失败",
        );
      }
    },
    {
      body: t.Object({
        enabled: t.Boolean(),
        max_days: t.Number(),
      }),
    },
  )
  .get("/directory", async ({ set }) => {
    const response = await goBackend.getGatewayLoggingDirectory();
    if (!response.success || !response.data) {
      return toFailure(set, response.message || "读取日志目录失败");
    }
    return { success: true, data: response.data };
  })
  .get("/dates", async ({ set }) => {
    const response = await goBackend.getGatewayLogDates();
    if (!response.success || !response.data) {
      return toFailure(set, response.message || "读取日志日期失败");
    }
    return { success: true, data: response.data };
  })
  .get(
    "/entries",
    async ({ query, set }) => {
      const response = await goBackend.getGatewayLogEntries(query);
      if (!response.success || !response.data) {
        return toFailure(set, response.message || "读取请求日志失败", 400);
      }
      return { success: true, data: response.data };
    },
    {
      query: t.Object({
        date: t.Optional(t.String()),
        pagination: t.Optional(t.String()),
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        cursor: t.Optional(t.String()),
        search: t.Optional(t.String()),
        status: t.Optional(t.String()),
        logged_in: t.Optional(t.String()),
      }),
    },
  )
  .delete(
    "/entries",
    async ({ body, set }) => {
      const response = await goBackend.deleteGatewayLogEntries(body.date);
      if (!response.success || !response.data) {
        return toFailure(set, response.message || "删除请求日志失败", 400);
      }
      return { success: true, data: response.data };
    },
    {
      body: t.Object({
        date: t.String(),
      }),
    },
  );
