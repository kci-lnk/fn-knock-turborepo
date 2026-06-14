import { Elysia, t } from "elysia";
import { resetAuthFailureTracking } from "../lib/auth-failure";
import { loginBackoffService } from "../lib/login-backoff";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const getBackoffTranslator = async (request: Request) =>
  createRequestTranslator(request, await configManager.getLocaleConfig());

export const backoffRoutes = new Elysia({
  prefix: "/api/admin/backoff",
  tags: ["Backoff"],
})
  .get(
    "/list",
    async () => {
      const items = await loginBackoffService.listBlocked();
      return { success: true, data: items };
    },
    routeDoc("获取登录退避封禁列表"),
  )
  .get(
    "/status",
    async ({ request, query, set }) => {
      const { t } = await getBackoffTranslator(request);
      const ip = query.ip as string | undefined;
      if (!ip) {
        set.status = 400;
        return { success: false, message: t("server.backoffRoutes.ipRequired") };
      }
      const st = await loginBackoffService.getStatus(ip);
      return { success: true, data: st };
    },
    routeDoc("查询指定 IP 的退避状态"),
  )
  .post(
    "/reset",
    async ({ body }) => {
      await resetAuthFailureTracking(body.ip);
      return { success: true };
    },
    withRouteDoc("重置指定 IP 的退避记录", {
      body: t.Object({
        ip: t.String(),
      }),
    }),
  );
