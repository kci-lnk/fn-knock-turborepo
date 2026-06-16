import { Elysia, t } from "elysia";
import {
  isSystemEventLevel,
  isSystemEventSource,
  isSystemEventType,
} from "../lib/system-events/constants";
import { hydrateSystemEventIpLocations } from "../lib/system-events/ip-locations";
import { systemEventManager } from "../lib/system-events/manager";
import { withRouteDoc } from "../lib/openapi";

const parsePositiveInt = (value: string | undefined, fallback: number) => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
};

export const eventRoutes = new Elysia({
  prefix: "/api/admin/events",
  tags: ["Events"],
})
  .get(
    "/",
    async ({ query, set }) => {
      const type = query.type?.trim();
      const level = query.level?.trim();
      const source = query.source?.trim();
      const eventType = type && isSystemEventType(type) ? type : undefined;
      const eventLevel = level && isSystemEventLevel(level) ? level : undefined;
      const eventSource =
        source && isSystemEventSource(source) ? source : undefined;

      if (type && !isSystemEventType(type)) {
        set.status = 400;
        return { success: false, message: "Unsupported event type" };
      }
      if (level && !isSystemEventLevel(level)) {
        set.status = 400;
        return { success: false, message: "Unsupported event level" };
      }
      if (source && !isSystemEventSource(source)) {
        set.status = 400;
        return { success: false, message: "Unsupported event source" };
      }

      const result = await systemEventManager.list({
        page: parsePositiveInt(query.page, 1),
        limit: Math.min(parsePositiveInt(query.limit, 20), 100),
        search: query.search || "",
        ...(eventType ? { type: eventType } : {}),
        ...(eventLevel ? { level: eventLevel } : {}),
        ...(eventSource ? { source: eventSource } : {}),
      });
      await hydrateSystemEventIpLocations(result.events);

      return { success: true, data: result };
    },
    withRouteDoc("分页查询系统事件", {
      query: t.Object({
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        search: t.Optional(t.String()),
        type: t.Optional(t.String()),
        level: t.Optional(t.String()),
        source: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "/clear",
    async () => {
      const deletedCount = await systemEventManager.clearAll();
      return {
        success: true,
        data: {
          deleted_count: deletedCount,
        },
      };
    },
    withRouteDoc("清空系统事件", {}),
  )
  .delete(
    "/",
    async ({ body }) => {
      await systemEventManager.deleteMany(body.ids);
      return { success: true };
    },
    withRouteDoc("批量删除系统事件", {
      body: t.Object({
        ids: t.Array(t.String()),
      }),
    }),
  );
