import { Elysia, t } from "elysia";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import {
  FrpcConfigValidationError,
  FrpcInstanceLimitError,
  FrpcInstanceNotFoundError,
  frpcInstanceManager,
} from "../lib/frpc/manager";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const getFrpcRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

const parseLimit = (
  value: unknown,
  fallback: number,
  max: number,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(1, Math.min(parsed, max));
};

const setErrorResponse = (
  set: { status?: number | string },
  error: unknown,
  fallback: string,
) => {
  const message = error instanceof Error ? error.message : fallback;
  if (error instanceof FrpcConfigValidationError) {
    set.status = 400;
  } else if (error instanceof FrpcInstanceNotFoundError) {
    set.status = 404;
  } else if (error instanceof FrpcInstanceLimitError) {
    set.status = 400;
  } else {
    set.status = 500;
  }
  return { success: false, message };
};

export async function restoreFrpcOnBoot(): Promise<void> {
  await frpcInstanceManager.restoreOnBoot();
}

export const frpcRoutes = new Elysia({
  prefix: "/api/admin/frpc",
  tags: ["Tunnel - FRP"],
})
  .get(
    "/status",
    async () => {
      const overview = await frpcInstanceManager.getOverview();
      const primary =
        overview.items.find((item) => item.id === overview.primaryInstanceId) ??
        null;
      return {
        success: true,
        data: {
          initialized: overview.initialized,
          platform: overview.platform,
          running: primary?.running ?? false,
          pid: primary?.pid ?? null,
          config_path: primary?.configPath ?? "",
          defaults: overview.defaults,
          total: overview.total,
          running_count: overview.runningCount,
        },
      };
    },
    routeDoc("获取 FRP 客户端状态"),
  )
  .get(
    "/overview",
    async ({ query }) => {
      const logs = await frpcInstanceManager.listLogs(
        frpcInstanceManager.primaryId,
        parseLimit(query.limit, 200, 1000),
      );
      return { success: true, data: { tcp: [], logs } };
    },
    routeDoc("获取 FRP 总览信息"),
  )
  .get(
    "/web-status",
    async () => {
      return { success: true, data: { tcp: [] } };
    },
    routeDoc("获取 FRP Web 管理状态"),
  )
  .get(
    "/config",
    async () => {
      const content = await frpcInstanceManager.readConfig(
        frpcInstanceManager.primaryId,
      );
      return { success: true, data: { content } };
    },
    routeDoc("获取 FRP 配置文件"),
  )
  .post(
    "/config",
    async ({ body, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        await frpcInstanceManager.saveConfig(
          frpcInstanceManager.primaryId,
          body.content,
        );
        return { success: true };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.saveConfigFailed"));
      }
    },
    withRouteDoc("保存 FRP 配置文件", {
      body: t.Object({ content: t.String() }),
    }),
  )
  .post(
    "/start",
    async ({ set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const { pid } = await frpcInstanceManager.start(
          frpcInstanceManager.primaryId,
        );
        return { success: true, data: { pid } };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.startFailed"));
      }
    },
    routeDoc("启动 FRP 客户端"),
  )
  .post(
    "/stop",
    async ({ set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        await frpcInstanceManager.stop(frpcInstanceManager.primaryId);
        return { success: true };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.stopFailed"));
      }
    },
    routeDoc("停止 FRP 客户端"),
  )
  .get(
    "/logs",
    async ({ query }) => {
      const logs = await frpcInstanceManager.listLogs(
        frpcInstanceManager.primaryId,
        parseLimit(query.limit, 200, 1000),
      );
      return { success: true, data: logs };
    },
    routeDoc("获取 FRP 日志"),
  )
  .delete(
    "/logs",
    async () => {
      await frpcInstanceManager.clearLogs(frpcInstanceManager.primaryId);
      return { success: true };
    },
    routeDoc("清空 FRP 日志"),
  )
  .get(
    "/poll",
    async ({ query }) => {
      const [poll, overview] = await Promise.all([
        frpcInstanceManager.poll(frpcInstanceManager.primaryId, query.cursor),
        frpcInstanceManager.getOverview(),
      ]);
      return {
        success: true,
        data: {
          ...poll,
          status: {
            ...poll.status,
            tcp: [],
            instances: overview,
          },
        },
      };
    },
    withRouteDoc("轮询 FRP 日志与状态", {
      query: t.Object({
        cursor: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/instances",
    async () => {
      const overview = await frpcInstanceManager.getOverview();
      return { success: true, data: overview };
    },
    routeDoc("获取 FRP 实例列表"),
  )
  .post(
    "/instances/draft",
    async () => {
      return {
        success: true,
        data: { content: frpcInstanceManager.defaultContent() },
      };
    },
    routeDoc("生成 FRP 实例草稿配置"),
  )
  .post(
    "/instances",
    async ({ body, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const item = await frpcInstanceManager.createInstance({
          name: body.name,
          content: body.content,
        });
        return { success: true, data: item };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.createInstanceFailed"));
      }
    },
    withRouteDoc("创建 FRP 实例", {
      body: t.Object({
        name: t.Optional(t.String()),
        content: t.Optional(t.String()),
      }),
    }),
  )
  .post(
    "/instances/:id/start",
    async ({ params, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const { pid } = await frpcInstanceManager.start(params.id);
        return { success: true, data: { pid } };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.startInstanceFailed"));
      }
    },
    routeDoc("启动指定 FRP 实例"),
  )
  .post(
    "/instances/:id/stop",
    async ({ params, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        await frpcInstanceManager.stop(params.id);
        return { success: true };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.stopInstanceFailed"));
      }
    },
    routeDoc("停止指定 FRP 实例"),
  )
  .post(
    "/instances/:id/restart",
    async ({ params, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const { pid } = await frpcInstanceManager.restart(params.id);
        return { success: true, data: { pid } };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.restartInstanceFailed"));
      }
    },
    routeDoc("重启指定 FRP 实例"),
  )
  .get(
    "/instances/:id/logs",
    async ({ params, query, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const logs = await frpcInstanceManager.listLogs(
          params.id,
          parseLimit(query.limit, 200, 1000),
        );
        return { success: true, data: logs };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.getInstanceLogsFailed"));
      }
    },
    routeDoc("获取指定 FRP 实例日志"),
  )
  .delete(
    "/instances/:id/logs",
    async ({ params, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        await frpcInstanceManager.clearLogs(params.id);
        return { success: true };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.clearInstanceLogsFailed"));
      }
    },
    routeDoc("清空指定 FRP 实例日志"),
  )
  .get(
    "/instances/:id/poll",
    async ({ params, query, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const data = await frpcInstanceManager.poll(params.id, query.cursor);
        return { success: true, data };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.pollInstanceFailed"));
      }
    },
    withRouteDoc("轮询指定 FRP 实例日志与状态", {
      query: t.Object({
        cursor: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/instances/:id",
    async ({ params, query, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const detail = await frpcInstanceManager.getDetail(
          params.id,
          parseLimit(query.limit, 200, 1000),
        );
        return { success: true, data: detail };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.getInstanceDetailFailed"));
      }
    },
    routeDoc("获取 FRP 实例详情"),
  )
  .put(
    "/instances/:id",
    async ({ params, body, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        const item = await frpcInstanceManager.updateInstance(params.id, {
          name: body.name,
          content: body.content,
        });
        return { success: true, data: item };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.updateInstanceFailed"));
      }
    },
    withRouteDoc("更新 FRP 实例", {
      body: t.Object({
        name: t.Optional(t.String()),
        content: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "/instances/:id",
    async ({ params, set, request }) => {
      const { t } = await getFrpcRouteTranslator(request);
      try {
        await frpcInstanceManager.deleteInstance(params.id);
        return { success: true };
      } catch (error) {
        return setErrorResponse(set, error, t("server.frpc.routes.deleteInstanceFailed"));
      }
    },
    routeDoc("删除 FRP 实例"),
  );
