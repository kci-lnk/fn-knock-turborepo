import { Elysia, t } from "elysia";
import { oidcAuthService } from "../lib/auth/oidc/service";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const getOIDCAdminRouteTranslator = async (request: Request) => {
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

export const oidcAdminRoutes = new Elysia({
  prefix: "/api/admin/auth/oidc",
  tags: ["Admin"],
})
  .get(
    "/catalog",
    () => ({
      success: true,
      data: {
        providers: oidcAuthService.listProviderCatalog(),
      },
    }),
    routeDoc("获取外部登录提供商目录"),
  )
  .get(
    "/providers",
    async ({ request }) => ({
      success: true,
      data: {
        providers: await oidcAuthService.listProviders(request),
      },
    }),
    routeDoc("获取外部登录提供商列表"),
  )
  .post(
    "/providers",
    async ({ body, set, request }) => {
      const { t } = await getOIDCAdminRouteTranslator(request);
      try {
        const provider = await oidcAuthService.createProvider(body);
        return { success: true, data: provider };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.createProviderFailed"),
        };
      }
    },
    withRouteDoc("创建外部登录提供商", { body: providerCreateBody }),
  )
  .patch(
    "/providers/:id",
    async ({ params, body, set, request }) => {
      const { t } = await getOIDCAdminRouteTranslator(request);
      try {
        const provider = await oidcAuthService.updateProvider(params.id, body);
        return { success: true, data: provider };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.updateProviderFailed"),
        };
      }
    },
    withRouteDoc("更新外部登录提供商", { body: providerUpdateBody }),
  )
  .delete(
    "/providers/:id",
    async ({ params, set, request }) => {
      const { t } = await getOIDCAdminRouteTranslator(request);
      try {
        await oidcAuthService.deleteProvider(params.id);
        return { success: true };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.deleteProviderFailed"),
        };
      }
    },
    routeDoc("删除外部登录提供商"),
  )
  .post(
    "/providers/:id/test",
    async ({ params, set, request }) => {
      const { t } = await getOIDCAdminRouteTranslator(request);
      try {
        return await oidcAuthService.testProvider(params.id);
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.testProviderFailed"),
        };
      }
    },
    routeDoc("测试外部登录提供商"),
  )
  .get(
    "/totp/:totpId/bindings",
    async ({ params }) => ({
      success: true,
      data: {
        bindings: await oidcAuthService.listBindingsByTotp(params.totpId),
      },
    }),
    routeDoc("获取 TOTP 关联的外部账号绑定"),
  )
  .delete(
    "/bindings/:id",
    async ({ params, set, request }) => {
      const { t } = await getOIDCAdminRouteTranslator(request);
      try {
        await oidcAuthService.deleteBinding(params.id);
        return { success: true };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.deleteBindingFailed"),
        };
      }
    },
    routeDoc("删除外部账号绑定"),
  )
  .post(
    "/invitations",
    async ({ body, request, set }) => {
      const { t } = await getOIDCAdminRouteTranslator(request);
      try {
        const result = await oidcAuthService.createInvite({
          request,
          totpId: body.totp_id,
          providerId: body.provider_id,
          note: body.note,
        });
        return {
          success: true,
          data: {
            invite_url: result.invite_url,
            expires_at: result.invite.expires_at,
          },
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.oidc.createInviteFailed"),
        };
      }
    },
    withRouteDoc("创建外部账号绑定邀请", {
      body: t.Object({
        totp_id: t.String(),
        provider_id: t.String(),
        note: t.Optional(t.String()),
      }),
    }),
  );
