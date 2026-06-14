import { Elysia, t } from "elysia";
import { sshSecurityService } from "../lib/ssh-security/service";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

type RequestTranslator = ReturnType<typeof createRequestTranslator>["t"];

const getSshSecurityTranslator = async (request: Request) => {
  const locale = await configManager.getLocaleConfig();
  return createRequestTranslator(request, locale);
};

const sshSecurityRouteT = (
  t: RequestTranslator,
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => t(`server.sshSecurity.routes.${key}`, params);

const parseDeleteIps = (body: unknown): string[] => {
  if (!body || typeof body !== "object") return [];
  const value = (body as { ips?: unknown }).ips;
  if (!Array.isArray(value)) return [];
  return value.map((item) => String(item ?? "").trim()).filter(Boolean);
};

export const sshSecurityRoutes = new Elysia({
  prefix: "/api/admin/ssh-security",
  tags: ["SSH Security"],
})
  .get(
    "/config",
    async () => ({
      success: true,
      data: await sshSecurityService.getDetails(),
    }),
    routeDoc("获取 SSH 安全配置"),
  )
  .post(
    "/config",
    async ({ request, body, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      try {
        const details =
          body && Object.keys(body).length === 1 && "enabled" in body
            ? await sshSecurityService.patchEnabled(body.enabled === true)
            : await sshSecurityService.updateConfig({
                enabled: body.enabled,
                window_minutes: body.window_minutes,
                failed_login_threshold: body.failed_login_threshold,
                block_duration_value: body.block_duration_value,
                block_duration_unit: body.block_duration_unit,
                allowed_regions: body.allowed_regions,
                custom_cidrs: body.custom_cidrs,
              });
        return { success: true, data: details };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : sshSecurityRouteT(t, "updateConfigFailed"),
        };
      }
    },
    withRouteDoc("更新 SSH 安全配置", {
      body: t.Partial(
        t.Object({
          enabled: t.Boolean(),
          window_minutes: t.Number(),
          failed_login_threshold: t.Number(),
          block_duration_value: t.Number(),
          block_duration_unit: t.Union([
            t.Literal("minute"),
            t.Literal("hour"),
            t.Literal("day"),
          ]),
          allowed_regions: t.Array(
            t.Object({
              province: t.String(),
              query_city: t.Optional(t.Union([t.String(), t.Null()])),
            }),
          ),
          custom_cidrs: t.Array(t.String()),
        }),
      ),
    }),
  )
  .post(
    "/firewall/sync",
    async ({ request, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      try {
        const result = await sshSecurityService.syncFirewallBlocks();
        return {
          success: true,
          data: result,
          message: sshSecurityRouteT(t, "syncFirewallSuccess", {
            allowedCidrs: result.allowed_cidrs,
            ports: result.ports.join(", "),
            synced: result.synced,
          }),
        };
      } catch (error) {
        set.status = 502;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : sshSecurityRouteT(t, "syncFirewallFailed"),
        };
      }
    },
    routeDoc("同步 SSH 防火墙封锁规则"),
  )
  .post(
    "/firewall/clear",
    async ({ request, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      try {
        const result = await sshSecurityService.clearFirewall();
        return {
          success: true,
          data: result,
          message: sshSecurityRouteT(t, "clearFirewallSuccess"),
        };
      } catch (error) {
        set.status = 502;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : sshSecurityRouteT(t, "clearFirewallFailed"),
        };
      }
    },
    routeDoc("清空 SSH 专用防火墙规则"),
  )
  .get(
    "/login-logs",
    async ({ request, query, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      try {
        return {
          success: true,
          data: await sshSecurityService.listLoginLogs(query),
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : sshSecurityRouteT(t, "readLoginLogsFailed"),
        };
      }
    },
    routeDoc("查询 SSH 登录日志"),
  )
  .get(
    "/blocks",
    async ({ query }) => ({
      success: true,
      data: await sshSecurityService.listBlocks(query),
    }),
    routeDoc("查询 SSH 封锁列表"),
  )
  .get(
    "/blocks/:ip",
    async ({ request, params, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      const record = await sshSecurityService.getBlock(params.ip);
      if (!record) {
        set.status = 404;
        return { success: false, message: sshSecurityRouteT(t, "blockNotFound") };
      }
      return { success: true, data: record };
    },
    withRouteDoc("查询 SSH 封锁详情", {
      params: t.Object({
        ip: t.String(),
      }),
    }),
  )
  .delete(
    "/blocks/:ip",
    async ({ request, params, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      try {
        const removed = await sshSecurityService.removeBlock(params.ip);
        if (!removed) {
          set.status = 404;
          return {
            success: false,
            message: sshSecurityRouteT(t, "blockNotFound"),
          };
        }
        return { success: true };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : sshSecurityRouteT(t, "removeBlockFailed"),
        };
      }
    },
    withRouteDoc("解除单个 SSH 封锁", {
      params: t.Object({
        ip: t.String(),
      }),
    }),
  )
  .delete(
    "/blocks",
    async ({ request, body, set }) => {
      const { t } = await getSshSecurityTranslator(request);
      const ips = parseDeleteIps(body);
      if (ips.length === 0) {
        set.status = 400;
        return { success: false, message: sshSecurityRouteT(t, "selectIps") };
      }

      try {
        const removed = await sshSecurityService.removeBlocks(ips);
        return { success: true, data: { removed } };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : sshSecurityRouteT(t, "removeBlocksFailed"),
        };
      }
    },
    routeDoc("批量解除 SSH 封锁"),
  );
