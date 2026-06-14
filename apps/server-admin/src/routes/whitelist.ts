import { Elysia, t } from "elysia";
import { scheduleSyncReverseProxyTrustedIPs } from "../lib/reverse-proxy-trusted-ips";
import { whitelistManager } from "../lib/whitelist-manager";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";
import { normalizeAutoIpGrantComment } from "../lib/post-login-ip-grant";

const getWhitelistRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

export const whitelistRoutes = new Elysia({
  prefix: "/api/admin/whitelist",
  tags: ["Whitelist"],
})
  .get(
    "/",
    async ({ request }) => {
      const { locale } = await getWhitelistRouteTranslator(request);
      const records = await whitelistManager.getAllActiveRecords();
      return {
        success: true,
        data: records.map((record) => ({
          ...record,
          ...(record.comment !== undefined
            ? {
                comment: normalizeAutoIpGrantComment(record.comment, locale),
              }
            : {}),
        })),
      };
    },
    routeDoc("获取白名单列表"),
  )
  .post(
    "/",
    async ({ body, set, request }) => {
      const { t } = await getWhitelistRouteTranslator(request);
      try {
        const id = await whitelistManager.addWhiteList({
          ip: body.ip,
          targetType: body.targetType,
          expireAt: body.expireAt,
          source: body.source,
          comment: body.comment,
          checkIntervalMinutes: body.checkIntervalMinutes,
        });
        scheduleSyncReverseProxyTrustedIPs({ reason: "whitelist-add" });
        return { success: true, data: { id } };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.whitelist.addFailed"),
        };
      }
    },
    withRouteDoc("新增白名单记录", {
      body: t.Object({
        ip: t.String(),
        targetType: t.Optional(
          t.Union([t.Literal("ip"), t.Literal("cidr"), t.Literal("cname")]),
        ),
        expireAt: t.Union([t.Number(), t.Null()]),
        source: t.Union([t.Literal("manual"), t.Literal("auto")]),
        comment: t.Optional(t.String()),
        checkIntervalMinutes: t.Optional(t.Number()),
      }),
    }),
  )
  .delete(
    "/:id",
    async ({ params, set, request }) => {
      const { t } = await getWhitelistRouteTranslator(request);
      const deleted = await whitelistManager.removeWhiteList(params.id);
      if (!deleted) {
        set.status = 404;
        return {
          success: false,
          message: t("server.whitelist.recordNotFound"),
        };
      }
      scheduleSyncReverseProxyTrustedIPs({ reason: "whitelist-remove" });
      return { success: true };
    },
    withRouteDoc("删除白名单记录", {
      params: t.Object({
        id: t.String(),
      }),
    }),
  )
  .patch(
    "/:id/comment",
    async ({ params, body, set, request }) => {
      const { t } = await getWhitelistRouteTranslator(request);
      const updated = await whitelistManager.updateComment(
        params.id,
        body.comment,
      );
      if (!updated) {
        set.status = 404;
        return {
          success: false,
          message: t("server.whitelist.recordNotFound"),
        };
      }
      return { success: true };
    },
    withRouteDoc("更新白名单备注", {
      params: t.Object({
        id: t.String(),
      }),
      body: t.Object({
        comment: t.String(),
      }),
    }),
  )
  .post(
    "/:id/refresh",
    async ({ params, set, request }) => {
      const { t } = await getWhitelistRouteTranslator(request);
      try {
        const result = await whitelistManager.refreshCnameRecord(params.id, {
          force: true,
        });
        if (!result) {
          set.status = 404;
          return {
            success: false,
            message: t("server.whitelist.recordNotFound"),
          };
        }

        if (result.changed) {
          scheduleSyncReverseProxyTrustedIPs({ reason: "whitelist-refresh" });
        }
        if (result.record.resolveStatus === "error") {
          return {
            success: false,
            message:
              result.record.resolveMessage ||
              t("server.whitelist.domainResolveFailed"),
            data: result,
          };
        }
        if (result.syncError) {
          return {
            success: false,
            message: result.syncError,
            data: result,
          };
        }
        return {
          success: true,
          data: result,
        };
      } catch (error: any) {
        set.status = 400;
        return {
          success: false,
          message: error?.message || t("server.whitelist.refreshFailed"),
        };
      }
    },
    withRouteDoc("立即更新域名白名单记录", {
      params: t.Object({
        id: t.String(),
      }),
    }),
  );
