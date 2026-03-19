import { Elysia, t } from "elysia";
import { goBackend } from "../lib/go-backend";

const toFailure = (
  set: { status?: number },
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
  .get("/config", async ({ set }) => {
    const response = await goBackend.getGatewayLoggingConfig();
    if (!response.success || !response.data) {
      return toFailure(set, response.message || "读取网关日志配置失败");
    }
    return { success: true, data: response.data };
  })
  .post(
    "/config",
    async ({ body, set }) => {
      const response = await goBackend.setGatewayLoggingConfig(body);
      if (!response.success || !response.data) {
        return toFailure(set, response.message || "保存网关日志配置失败", 400);
      }
      return { success: true, data: response.data };
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
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        search: t.Optional(t.String()),
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
