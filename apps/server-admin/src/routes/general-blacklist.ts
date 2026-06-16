import { Elysia, t } from "elysia";
import {
  goBackend,
  type GeneralBlacklistSource,
  type GoResponse,
} from "../lib/go-backend";
import { normalizeIp } from "../lib/ip-normalize";
import { withRouteDoc } from "../lib/openapi";

const GENERAL_BLACKLIST_SOURCES = new Set<GeneralBlacklistSource>([
  "manual",
  "request_log",
  "active_ip",
  "waf_log",
]);

export const normalizeGeneralBlacklistSource = (
  source: unknown,
): GeneralBlacklistSource => {
  const candidate = String(source ?? "").trim();
  return GENERAL_BLACKLIST_SOURCES.has(candidate as GeneralBlacklistSource)
    ? (candidate as GeneralBlacklistSource)
    : "manual";
};

const parseBody = (body: unknown): unknown => {
  if (typeof body !== "string") return body;
  return JSON.parse(body) as unknown;
};

export const normalizeGeneralBlacklistIpList = (value: unknown): string[] => {
  const rawItems = Array.isArray(value) ? value : [];
  const seen = new Set<string>();

  for (const item of rawItems) {
    if (typeof item !== "string" || item.trim() === "") {
      throw new Error("Invalid IP");
    }
    const normalized = normalizeIp(item);
    if (!normalized) {
      throw new Error(`Invalid IP: ${item.trim()}`);
    }
    seen.add(normalized);
  }

  return Array.from(seen);
};

export const normalizeGeneralBlacklistStatusIpList = (
  value: unknown,
): string[] => {
  const rawItems = Array.isArray(value) ? value : [];
  const seen = new Set<string>();

  for (const item of rawItems) {
    if (typeof item !== "string" || item.trim() === "") continue;
    const normalized = normalizeIp(item);
    if (!normalized) continue;
    seen.add(normalized);
  }

  return Array.from(seen);
};

export const parseGeneralBlacklistDeleteIps = (body: unknown): string[] => {
  const parsedBody = parseBody(body);
  if (Array.isArray(parsedBody)) {
    return normalizeGeneralBlacklistIpList(parsedBody);
  }
  if (
    parsedBody &&
    typeof parsedBody === "object" &&
    Array.isArray((parsedBody as { ips?: unknown }).ips)
  ) {
    return normalizeGeneralBlacklistIpList(
      (parsedBody as { ips: unknown[] }).ips,
    );
  }
  return [];
};

const responseStatus = (response: GoResponse): number =>
  response.code && response.code >= 400 ? response.code : 502;

const proxyFailure = (
  set: { status?: number | string },
  response: GoResponse,
) => {
  set.status = responseStatus(response);
  return {
    success: false,
    message: response.message || "Go backend request failed",
  };
};

export const generalBlacklistRoutes = new Elysia({
  prefix: "/api/admin/general-blacklist",
  tags: ["General Blacklist"],
})
  .get(
    "",
    async ({ query, set }) => {
      const response = await goBackend.getGeneralBlacklist({
        page: query.page || "1",
        limit: query.limit || "20",
        search: query.search || "",
      });
      if (!response.success || !response.data) {
        return proxyFailure(set, response);
      }
      return { success: true, data: response.data };
    },
    withRouteDoc("分页查询通用黑名单", {
      query: t.Object({
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        search: t.Optional(t.String()),
      }),
    }),
  )
  .post(
    "/status",
    async ({ body, set }) => {
      let parsedBody: unknown;
      try {
        parsedBody = parseBody(body);
      } catch {
        set.status = 400;
        return { success: false, message: "Invalid request body" };
      }

      const ips = normalizeGeneralBlacklistStatusIpList(
        (parsedBody as { ips?: unknown })?.ips,
      );

      const response = await goBackend.getGeneralBlacklistStatus(ips);
      if (!response.success || !response.data) {
        return proxyFailure(set, response);
      }
      return { success: true, data: response.data };
    },
    withRouteDoc("批量查询通用黑名单状态", {
      body: t.Object({
        ips: t.Array(t.String()),
      }),
    }),
  )
  .post(
    "",
    async ({ body, set }) => {
      let parsedBody: unknown;
      try {
        parsedBody = parseBody(body);
      } catch {
        set.status = 400;
        return { success: false, message: "Invalid request body" };
      }

      let ips: string[];
      try {
        ips = normalizeGeneralBlacklistIpList(
          (parsedBody as { ips?: unknown })?.ips,
        );
      } catch (error: any) {
        set.status = 400;
        return { success: false, message: error?.message || "Invalid IP" };
      }

      if (ips.length === 0) {
        set.status = 400;
        return { success: false, message: "At least one valid IP is required" };
      }

      const response = await goBackend.addGeneralBlacklist({
        ips,
        source: normalizeGeneralBlacklistSource(
          (parsedBody as { source?: unknown })?.source,
        ),
        comment:
          typeof (parsedBody as { comment?: unknown })?.comment === "string"
            ? (parsedBody as { comment: string }).comment.trim()
            : "",
      });

      if (!response.success || !response.data) {
        return proxyFailure(set, response);
      }
      return { success: true, data: response.data };
    },
    withRouteDoc("批量新增通用黑名单", {
      body: t.Object({
        ips: t.Array(t.String()),
        source: t.Optional(
          t.Union([
            t.Literal("manual"),
            t.Literal("request_log"),
            t.Literal("active_ip"),
            t.Literal("waf_log"),
          ]),
        ),
        comment: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "",
    async ({ body, set }) => {
      let ips: string[];
      try {
        ips = parseGeneralBlacklistDeleteIps(body);
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || "Invalid request body",
        };
      }

      if (ips.length === 0) {
        set.status = 400;
        return { success: false, message: "At least one valid IP is required" };
      }

      const response = await goBackend.deleteGeneralBlacklist(ips);
      if (!response.success || !response.data) {
        return proxyFailure(set, response);
      }
      return { success: true, data: response.data };
    },
    withRouteDoc("批量删除通用黑名单", {
      body: t.Optional(t.Any()),
    }),
  )
  .delete(
    "/:ip",
    async ({ params, set }) => {
      const ip = normalizeIp(params.ip);
      if (!ip) {
        set.status = 400;
        return { success: false, message: "Invalid IP" };
      }

      const response = await goBackend.deleteGeneralBlacklistByIp(ip);
      if (!response.success || !response.data) {
        return proxyFailure(set, response);
      }
      return { success: true, data: response.data };
    },
    withRouteDoc("删除指定通用黑名单 IP", {
      params: t.Object({
        ip: t.String(),
      }),
    }),
  );
