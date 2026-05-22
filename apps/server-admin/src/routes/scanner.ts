import { Elysia, t } from "elysia";
import { scanDetector } from "../lib/scan-detector";
import { routeDoc, withRouteDoc } from "../lib/openapi";

const parseBlacklistDeleteIps = (body: unknown): string[] => {
  const parsedBody =
    typeof body === "string" ? (JSON.parse(body) as unknown) : body;

  if (Array.isArray(parsedBody)) {
    return parsedBody.filter((ip): ip is string => typeof ip === "string");
  }

  if (
    parsedBody &&
    typeof parsedBody === "object" &&
    Array.isArray((parsedBody as { ips?: unknown }).ips)
  ) {
    return (parsedBody as { ips: unknown[] }).ips.filter(
      (ip): ip is string => typeof ip === "string",
    );
  }

  return [];
};

export const scannerRoutes = new Elysia({
  prefix: "/api/admin/scanner",
  tags: ["Scanner"],
})
  .get(
    "/settings",
    async () => {
      const settings = await scanDetector.getSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取扫描器设置"),
  )
  .post(
    "/settings",
    async ({ body }) => {
      const settings = await scanDetector.updateSettings({
        enabled: body.enabled,
        windowMinutes: body.windowMinutes,
        threshold: body.threshold,
        blacklistTtlSeconds: body.blacklistTtlSeconds,
        commonLocationExemptEnabled: body.commonLocationExemptEnabled,
      });
      return { success: true, data: settings };
    },
    withRouteDoc("更新扫描器设置", {
      body: t.Object({
        enabled: t.Boolean(),
        windowMinutes: t.Number(),
        threshold: t.Number(),
        blacklistTtlSeconds: t.Number(),
        commonLocationExemptEnabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/blacklist",
    async ({ query }) => {
      const page = parseInt(query.page || "1", 10);
      const limit = parseInt(query.limit || "20", 10);
      const search = query.search || "";
      const data = await scanDetector.listBlacklist({ page, limit, search });
      return { success: true, data };
    },
    withRouteDoc("分页查询扫描器黑名单", {
      query: t.Object({
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        search: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/blacklist/:ip",
    async ({ params, set }) => {
      const record = await scanDetector.getBlacklistRecord(params.ip);
      if (!record) {
        set.status = 404;
        return { success: false, message: "Record not found" };
      }
      return { success: true, data: record };
    },
    withRouteDoc("获取指定 IP 的黑名单记录", {
      params: t.Object({
        ip: t.String(),
      }),
    }),
  )
  .delete(
    "/blacklist/:ip",
    async ({ params }) => {
      await scanDetector.removeFromBlacklist(params.ip);
      return { success: true };
    },
    withRouteDoc("删除指定 IP 的黑名单记录", {
      params: t.Object({
        ip: t.String(),
      }),
    }),
  )
  .delete(
    "/blacklist",
    async ({ body, set }) => {
      let ips: string[];
      try {
        ips = parseBlacklistDeleteIps(body);
      } catch {
        set.status = 400;
        return { success: false, message: "Invalid request body" };
      }

      if (ips.length === 0) {
        set.status = 400;
        return { success: false, message: "At least one IP is required" };
      }

      await scanDetector.removeManyFromBlacklist(ips);
      return { success: true };
    },
    withRouteDoc("批量删除黑名单记录", {
      body: t.Optional(t.Any()),
    }),
  );
