import { Elysia, t } from "elysia";
import { systemNotificationService } from "../lib/system-notifications/service";
import {
  NOTIFICATION_DELIVERY_STATUSES,
  NOTIFICATION_TRIGGER_STATUSES,
} from "../lib/system-notifications/types";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const parsePositiveInt = (value: string | undefined, fallback: number) => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
};

const getNotificationRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

const providerCreateBody = t.Object({
  name: t.Optional(t.String()),
  type: t.String(),
  enabled: t.Optional(t.Boolean()),
  connection_config: t.Optional(t.Record(t.String(), t.Any())),
});

const providerUpdateBody = t.Object({
  name: t.Optional(t.String()),
  enabled: t.Optional(t.Boolean()),
  connection_config: t.Optional(t.Record(t.String(), t.Any())),
});

const providerDraftTestBody = t.Object({
  id: t.Optional(t.String()),
  name: t.Optional(t.String()),
  type: t.String(),
  enabled: t.Optional(t.Boolean()),
  connection_config: t.Optional(t.Record(t.String(), t.Any())),
});

const nullableRecord = t.Union([t.Record(t.String(), t.Any()), t.Null()]);

const ruleBody = t.Object({
  name: t.Optional(t.String()),
  enabled: t.Optional(t.Boolean()),
  event_type: t.String(),
  event_level_filter: t.Optional(t.Array(t.String())),
  event_source_filter: t.Optional(t.Array(t.String())),
  window_seconds: t.Number(),
  threshold_count: t.Number(),
  group_by: t.String(),
  cooldown_seconds: t.Number(),
  targets: t.Array(
    t.Object({
      id: t.Optional(t.String()),
      provider_id: t.String(),
      enabled: t.Optional(t.Boolean()),
      target_config: t.Optional(t.Record(t.String(), t.Any())),
      template_override_mode: t.Optional(t.String()),
      template_override: t.Optional(nullableRecord),
      delivery_policy: t.Optional(t.Record(t.String(), t.Any())),
    }),
  ),
  message_template_mode: t.Optional(t.String()),
  message_template: t.Optional(nullableRecord),
});

const ruleUpdateBody = t.Object({
  name: t.Optional(t.String()),
  enabled: t.Optional(t.Boolean()),
  event_type: t.Optional(t.String()),
  event_level_filter: t.Optional(t.Array(t.String())),
  event_source_filter: t.Optional(t.Array(t.String())),
  window_seconds: t.Optional(t.Number()),
  threshold_count: t.Optional(t.Number()),
  group_by: t.Optional(t.String()),
  cooldown_seconds: t.Optional(t.Number()),
  targets: t.Optional(
    t.Array(
      t.Object({
        id: t.Optional(t.String()),
        provider_id: t.String(),
        enabled: t.Optional(t.Boolean()),
        target_config: t.Optional(t.Record(t.String(), t.Any())),
        template_override_mode: t.Optional(t.String()),
        template_override: t.Optional(nullableRecord),
        delivery_policy: t.Optional(t.Record(t.String(), t.Any())),
      }),
    ),
  ),
  message_template_mode: t.Optional(t.String()),
  message_template: t.Optional(nullableRecord),
});

const deliveryClearBody = t.Object({
  rule_id: t.Optional(t.String()),
  provider_id: t.Optional(t.String()),
  trigger_id: t.Optional(t.String()),
  status: t.Optional(t.String()),
});

export const notificationRoutes = new Elysia({
  prefix: "/api/admin/notifications",
  tags: ["Notifications"],
})
  .get(
    "/providers/catalog",
    async ({ request }) => {
      const { locale } = await getNotificationRouteTranslator(request);
      return {
        success: true,
        data: {
          providers: systemNotificationService.listProviderCatalog(locale),
        },
      };
    },
    routeDoc("获取通知提供商目录"),
  )
  .get(
    "/providers",
    async () => ({
      success: true,
      data: {
        providers: await systemNotificationService.listProviders(),
      },
    }),
    routeDoc("获取通知提供商列表"),
  )
  .post(
    "/providers",
    async ({ body, set, request }) => {
      try {
        const provider = await systemNotificationService.createProvider(body);
        return { success: true, data: provider };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.createProviderFailed"),
        };
      }
    },
    withRouteDoc("创建通知提供商", { body: providerCreateBody }),
  )
  .post(
    "/providers/test",
    async ({ body, set, request }) => {
      try {
        return await systemNotificationService.testProviderDraft(body);
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.testProviderFailed"),
        };
      }
    },
    withRouteDoc("按草稿配置测试通知提供商", { body: providerDraftTestBody }),
  )
  .get(
    "/providers/:id",
    async ({ params, set, request }) => {
      try {
        const provider = await systemNotificationService.getProvider(params.id);
        return { success: true, data: provider };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.getProviderFailed"),
        };
      }
    },
    routeDoc("获取通知提供商详情"),
  )
  .patch(
    "/providers/:id",
    async ({ params, body, set, request }) => {
      try {
        const provider = await systemNotificationService.updateProvider(
          params.id,
          body,
        );
        return { success: true, data: provider };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.updateProviderFailed"),
        };
      }
    },
    withRouteDoc("更新通知提供商", { body: providerUpdateBody }),
  )
  .delete(
    "/providers/:id",
    async ({ params, set, request }) => {
      try {
        await systemNotificationService.deleteProvider(params.id);
        return { success: true };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.deleteProviderFailed"),
        };
      }
    },
    routeDoc("删除通知提供商"),
  )
  .post(
    "/providers/:id/test",
    async ({ params, set, request }) => {
      try {
        return await systemNotificationService.testProvider(params.id);
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.testProviderFailed"),
        };
      }
    },
    routeDoc("测试通知提供商"),
  )
  .get(
    "/rules",
    async () => ({
      success: true,
      data: {
        rules: await systemNotificationService.listRules(),
      },
    }),
    routeDoc("获取通知规则列表"),
  )
  .post(
    "/rules",
    async ({ body, set, request }) => {
      try {
        const rule = await systemNotificationService.createRule(body);
        return { success: true, data: rule };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.createRuleFailed"),
        };
      }
    },
    withRouteDoc("创建通知规则", { body: ruleBody }),
  )
  .patch(
    "/rules/:id",
    async ({ params, body, set, request }) => {
      try {
        const rule = await systemNotificationService.updateRule(
          params.id,
          body,
        );
        return { success: true, data: rule };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.updateRuleFailed"),
        };
      }
    },
    withRouteDoc("更新通知规则", { body: ruleUpdateBody }),
  )
  .delete(
    "/rules/:id",
    async ({ params, set, request }) => {
      try {
        await systemNotificationService.deleteRule(params.id);
        return { success: true };
      } catch (error) {
        const { t } = await getNotificationRouteTranslator(request);
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.deleteRuleFailed"),
        };
      }
    },
    routeDoc("删除通知规则"),
  )
  .get(
    "/triggers",
    async ({ query }) => {
      const status = query.status?.trim();
      const result = await systemNotificationService.listTriggers({
        page: parsePositiveInt(query.page, 1),
        limit: Math.min(parsePositiveInt(query.limit, 20), 100),
        ...(query.rule_id?.trim() ? { rule_id: query.rule_id.trim() } : {}),
        ...(status && NOTIFICATION_TRIGGER_STATUSES.includes(status as any)
          ? { status: status as any }
          : {}),
      });
      return { success: true, data: result };
    },
    withRouteDoc("分页查询通知触发记录", {
      query: t.Object({
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        rule_id: t.Optional(t.String()),
        status: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/deliveries",
    async ({ query }) => {
      const status = query.status?.trim();
      const result = await systemNotificationService.listDeliveries({
        page: parsePositiveInt(query.page, 1),
        limit: Math.min(parsePositiveInt(query.limit, 20), 100),
        ...(query.rule_id?.trim() ? { rule_id: query.rule_id.trim() } : {}),
        ...(query.provider_id?.trim()
          ? { provider_id: query.provider_id.trim() }
          : {}),
        ...(query.trigger_id?.trim()
          ? { trigger_id: query.trigger_id.trim() }
          : {}),
        ...(status && NOTIFICATION_DELIVERY_STATUSES.includes(status as any)
          ? { status: status as any }
          : {}),
      });
      return { success: true, data: result };
    },
    withRouteDoc("分页查询通知投递记录", {
      query: t.Object({
        page: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        rule_id: t.Optional(t.String()),
        provider_id: t.Optional(t.String()),
        trigger_id: t.Optional(t.String()),
        status: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "/deliveries",
    async ({ body, set, request }) => {
      const { t } = await getNotificationRouteTranslator(request);
      const status = body.status?.trim();

      if (status && !NOTIFICATION_DELIVERY_STATUSES.includes(status as any)) {
        set.status = 400;
        return {
          success: false,
          message: t("server.notifications.routes.unsupportedDeliveryStatus"),
        };
      }

      try {
        const deletedCount = await systemNotificationService.clearDeliveries({
          ...(body.rule_id?.trim() ? { rule_id: body.rule_id.trim() } : {}),
          ...(body.provider_id?.trim()
            ? { provider_id: body.provider_id.trim() }
            : {}),
          ...(body.trigger_id?.trim()
            ? { trigger_id: body.trigger_id.trim() }
            : {}),
          ...(status ? { status: status as any } : {}),
        });

        return {
          success: true,
          data: {
            deleted_count: deletedCount,
          },
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.notifications.routes.clearDeliveriesFailed"),
        };
      }
    },
    withRouteDoc("按条件清空通知投递记录", {
      body: deliveryClearBody,
    }),
  );
