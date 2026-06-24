import { Elysia, t } from "elysia";
import {
  goBackend,
  type GatewayLogEntriesResponse,
  type GatewayLogEntry,
  type GoResponse,
} from "../lib/go-backend";
import {
  getGatewayLoggingConfigForResponse,
  syncGatewayLoggingToGateway,
} from "../lib/gateway-logging";
import { configManager } from "../lib/redis";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { isWhitelistExemptIp, normalizeIp } from "../lib/ip-normalize";
import { createRequestTranslator } from "../lib/i18n";

const getGatewayLogsRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

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

const WAF_STATUS_FILTERS = new Set(["has_waf", "none"]);

const normalizeWAFStatusFilter = (value: unknown): string => {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase();
  return WAF_STATUS_FILTERS.has(normalized) ? normalized : "";
};

const normalizePositiveInteger = (
  value: unknown,
  fallback: number,
  max: number,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.min(max, parsed);
};

const normalizeOptionalCursor = (value: unknown): number | null => {
  if (value === undefined || value === null || String(value).trim() === "") {
    return null;
  }
  const parsed = Number.parseInt(String(value), 10);
  if (!Number.isFinite(parsed) || parsed < 0) return null;
  return parsed;
};

const gatewayLogHasWAFSignal = (entry: GatewayLogEntry): boolean =>
  Boolean(entry.waf_trace_id) ||
  Boolean(entry.waf_bundle) ||
  Boolean(entry.waf_action) ||
  entry.waf_blocked === true ||
  (Array.isArray(entry.waf_rule_ids) && entry.waf_rule_ids.length > 0);

const gatewayLogMatchesWAFStatus = (
  entry: GatewayLogEntry,
  status: string,
): boolean => {
  const hasWAF = gatewayLogHasWAFSignal(entry);

  switch (status) {
    case "has_waf":
      return hasWAF;
    case "none":
      return !hasWAF;
    default:
      return true;
  }
};

const splitForwardedIps = (value: string | null | undefined): string[] =>
  String(value ?? "")
    .split(",")
    .map((item) => normalizeIp(item.trim()))
    .filter(Boolean);

const pickPreferredIp = (candidates: string[]): string => {
  const publicIp = candidates.find((ip) => !isWhitelistExemptIp(ip));
  return publicIp || candidates[0] || "";
};

const inferGatewayLogClientIp = (entry: GatewayLogEntry): string => {
  if (entry.client_ip) {
    return normalizeIp(entry.client_ip) || entry.client_ip;
  }

  const providerCandidates = [
    ...splitForwardedIps(entry.eo_connecting_ip),
    ...splitForwardedIps(entry.ali_real_client_ip),
  ];
  const providerIp = pickPreferredIp(providerCandidates);
  if (providerIp) return providerIp;

  const remoteIp = normalizeIp(entry.remote_ip);
  const proxyHeaderCandidates = [
    ...splitForwardedIps(entry.x_forwarded_for),
    ...splitForwardedIps(entry.x_real_ip),
  ];
  if (remoteIp && isWhitelistExemptIp(remoteIp)) {
    const proxyHeaderIp = pickPreferredIp(proxyHeaderCandidates);
    if (proxyHeaderIp) return proxyHeaderIp;
  }

  return remoteIp || entry.remote_ip || "";
};

const hydrateGatewayLogEntry = (entry: GatewayLogEntry): GatewayLogEntry => ({
  ...entry,
  client_ip: inferGatewayLogClientIp(entry),
});

const hydrateGatewayLogEntriesResponse = (
  data: GatewayLogEntriesResponse,
): GatewayLogEntriesResponse => ({
  ...data,
  items: data.items.map(hydrateGatewayLogEntry),
});

const getGatewayLogEntriesWithWAFFilter = async (
  query: Record<string, unknown>,
  wafStatus: string,
): Promise<GoResponse<GatewayLogEntriesResponse>> => {
  const limit = normalizePositiveInteger(query.limit, 20, 200);
  const initialCursor = normalizeOptionalCursor(query.cursor);
  const items: GatewayLogEntry[] = [];
  let baseData: GatewayLogEntriesResponse | null = null;
  let rawCursor = initialCursor === null ? undefined : String(initialCursor);
  let nextCursor = "";
  let hasMore = false;

  for (let scans = 0; scans < 200; scans += 1) {
    const remaining = Math.max(limit - items.length, 1);
    const response = await goBackend.getGatewayLogEntries({
      ...query,
      pagination: "cursor",
      cursor: rawCursor,
      limit: remaining,
      waf_status: undefined,
    });

    if (!response.success || !response.data) {
      return response;
    }

    const data = response.data;
    baseData = baseData || data;

    for (const entry of data.items) {
      if (gatewayLogMatchesWAFStatus(entry, wafStatus)) {
        items.push(entry);
      }
    }

    if (items.length >= limit) {
      const candidateNextCursor = data.next_cursor || "";
      if (!data.has_more || !candidateNextCursor) break;

      nextCursor = candidateNextCursor;
      let lookaheadCursor: string | undefined = candidateNextCursor;
      for (let lookups = scans + 1; lookups < 200; lookups += 1) {
        const lookaheadResponse = await goBackend.getGatewayLogEntries({
          ...query,
          pagination: "cursor",
          cursor: lookaheadCursor,
          limit,
          waf_status: undefined,
        });

        if (!lookaheadResponse.success || !lookaheadResponse.data) {
          return lookaheadResponse;
        }

        const lookaheadData = lookaheadResponse.data;
        if (
          lookaheadData.items.some((entry) =>
            gatewayLogMatchesWAFStatus(entry, wafStatus),
          )
        ) {
          hasMore = true;
          break;
        }

        if (!lookaheadData.has_more || !lookaheadData.next_cursor) break;
        lookaheadCursor = lookaheadData.next_cursor;
      }
      break;
    }

    if (!data.has_more || !data.next_cursor) break;
    rawCursor = data.next_cursor;
  }

  const responseCursor =
    initialCursor === null ? baseData?.cursor : String(initialCursor);

  if (!baseData) {
    return {
      success: true,
      data: {
        date: String(query.date || ""),
        logs_dir: "",
        available_dates: [],
        pagination: "cursor",
        page: 1,
        limit,
        total: 0,
        cursor: responseCursor || "",
        next_cursor: "",
        has_more: false,
        items: [],
      },
    };
  }

  return {
    success: true,
    data: hydrateGatewayLogEntriesResponse({
      ...baseData,
      pagination: "cursor",
      page: 1,
      limit,
      total: items.length + (hasMore ? 1 : 0),
      cursor: responseCursor,
      next_cursor: nextCursor,
      has_more: hasMore,
      items,
    }),
  };
};

export const gatewayLogsRoutes = new Elysia({
  prefix: "/api/admin/gateway-logs",
  tags: ["Gateway Logs"],
})
  .get(
    "/config",
    async () => {
      const settings = await configManager.getGatewayLoggingConfig();
      return {
        success: true,
        data: await getGatewayLoggingConfigForResponse(settings),
      };
    },
    routeDoc("获取网关请求日志配置"),
  )
  .post(
    "/config",
    async ({ body, set, request }) => {
      const { t } = await getGatewayLogsRouteTranslator(request);
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
          error?.message || t("server.gatewayLogs.configSyncFailed"),
        );
      }
    },
    withRouteDoc("更新网关请求日志配置", {
      body: t.Object({
        enabled: t.Boolean(),
        max_days: t.Number(),
      }),
    }),
  )
  .get(
    "/directory",
    async ({ set, request }) => {
      const { t } = await getGatewayLogsRouteTranslator(request);
      const response = await goBackend.getGatewayLoggingDirectory();
      if (!response.success || !response.data) {
        return toFailure(
          set,
          response.message || t("server.gatewayLogs.readDirectoryFailed"),
        );
      }
      return { success: true, data: response.data };
    },
    routeDoc("获取网关日志目录"),
  )
  .get(
    "/dates",
    async ({ set, request }) => {
      const { t } = await getGatewayLogsRouteTranslator(request);
      const response = await goBackend.getGatewayLogDates();
      if (!response.success || !response.data) {
        return toFailure(
          set,
          response.message || t("server.gatewayLogs.readDatesFailed"),
        );
      }
      return { success: true, data: response.data };
    },
    routeDoc("获取可查询的日志日期"),
  )
  .get(
    "/entries",
    async ({ query, set, request }) => {
      const { t } = await getGatewayLogsRouteTranslator(request);
      const wafStatus = normalizeWAFStatusFilter(query.waf_status);
      const response = wafStatus
        ? await getGatewayLogEntriesWithWAFFilter(query, wafStatus)
        : await goBackend.getGatewayLogEntries(query);
      if (!response.success || !response.data) {
        return toFailure(
          set,
          response.message || t("server.gatewayLogs.readEntriesFailed"),
          400,
        );
      }
      return {
        success: true,
        data: hydrateGatewayLogEntriesResponse(response.data),
      };
    },
    withRouteDoc("分页查询网关请求日志", {
      query: t.Object({
        date: t.Optional(t.String()),
        pagination: t.Optional(t.String()),
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        cursor: t.Optional(t.String()),
        search: t.Optional(t.String()),
        status: t.Optional(t.String()),
        logged_in: t.Optional(t.String()),
        credential: t.Optional(t.String()),
        waf_status: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "/entries",
    async ({ body, set, request }) => {
      const { t } = await getGatewayLogsRouteTranslator(request);
      const response = await goBackend.deleteGatewayLogEntries(body.date);
      if (!response.success || !response.data) {
        return toFailure(
          set,
          response.message || t("server.gatewayLogs.deleteEntriesFailed"),
          400,
        );
      }
      return { success: true, data: response.data };
    },
    withRouteDoc("按日期删除网关请求日志", {
      body: t.Object({
        date: t.String(),
      }),
    }),
  );
