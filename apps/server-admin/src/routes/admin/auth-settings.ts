import { randomBytes } from "node:crypto";
import { Elysia, t } from "elysia";
import { generateSecret, generateURI, verifySync } from "otplib";
import { oidcAuthService } from "../../lib/auth/oidc/service";
import { authMobilitySessionManager } from "../../lib/auth-mobility-session";
import { APP_LOCAL_VERSION } from "../../lib/app-version";
import { goBackend } from "../../lib/go-backend";
import { configManager } from "../../lib/redis";
import { scheduleSyncReverseProxyTrustedIPs } from "../../lib/reverse-proxy-trusted-ips";
import { buildGatewayAuthConfig } from "../../lib/subdomain-mode";
import { whitelistManager } from "../../lib/whitelist-manager";
import {
  buildTOTPCredentialImportPlan,
  buildTOTPCredentialTransferPayload,
  TOTPCredentialTransferError,
} from "../../lib/totp-credential-transfer";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import {
  adminT,
  ensureGoResponseSuccess,
  getAdminRouteTranslator,
  type RequestTranslator,
} from "./shared";

const buildTOTPCredentialExportFilename = (exportedAt: string) =>
  `fn-knock-totp-credentials-${exportedAt.replace(/[:.]/g, "-")}.json`;

const refreshGatewayAuthRuntime = async (
  t: RequestTranslator,
): Promise<void> => {
  const config = await configManager.getConfig();
  ensureGoResponseSuccess(
    await goBackend.setAuthConfig(buildGatewayAuthConfig(config)),
    adminT(t, "hostMappings.syncAuthConfigFailed"),
  );
};

const clearAutoIpGrantsForTotpCredential = async (
  totpId: string,
): Promise<void> => {
  const sessions = await configManager.listSessions();
  let changed = false;

  for (const session of sessions) {
    if (session.data.totpId !== totpId) continue;

    const whitelistRecordIds = new Set<string>();
    if (session.data.postLoginIpGrantRecordId) {
      whitelistRecordIds.add(session.data.postLoginIpGrantRecordId);
    }
    for (const recordId of await authMobilitySessionManager.listSessionWhitelistRecordIds(
      session.id,
    )) {
      whitelistRecordIds.add(recordId);
    }

    for (const recordId of whitelistRecordIds) {
      changed = (await whitelistManager.removeWhiteList(recordId)) || changed;
    }

    if (
      session.data.grantType === "login_ip_grant" ||
      session.data.postLoginIpGrantMode ||
      session.data.postLoginIpGrantRecordId
    ) {
      await configManager.updateSession(session.id, {
        grantType: "browser_session",
        postLoginIpGrantMode: null,
        postLoginIpGrantRecordId: null,
      });
      changed = true;
    }
  }

  if (changed) {
    scheduleSyncReverseProxyTrustedIPs({
      reason: "totp-subdomain-access-updated",
      delayMs: 0,
    });
  }
};

const destroySessionsForTotpCredential = async (
  totpId: string,
): Promise<number> => {
  const sessions = await configManager.listSessions();
  let destroyed = 0;

  for (const session of sessions) {
    if (session.data.totpId !== totpId) continue;
    await authMobilitySessionManager.destroySession(session.id);
    await configManager.deleteSession(session.id);
    destroyed += 1;
  }

  if (destroyed > 0) {
    scheduleSyncReverseProxyTrustedIPs({
      reason: "totp-credential-deleted",
      delayMs: 0,
    });
  }

  return destroyed;
};

export const adminAuthSettingsRoutes = new Elysia()
  .get(
    "/config/auth_credential_settings",
    async () => {
      const settings = await configManager.getAuthCredentialSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取认证凭据配置"),
  )
  .post(
    "/config/auth_credential_settings",
    async ({ body }) => {
      const previous = await configManager.getAuthCredentialSettings();
      const next =
        await configManager.previewAuthCredentialSettingsUpdate(body);
      const sessionIpMobilityChanged =
        previous.session_ip_mobility_enabled !==
          next.session_ip_mobility_enabled ||
        previous.session_ip_mobility_window_seconds !==
          next.session_ip_mobility_window_seconds;
      if (sessionIpMobilityChanged) {
        await authMobilitySessionManager.reconcileSessionIpMobilityPolicy(
          previous,
          next,
          { scheduleSync: false },
        );
      }
      let saved = next;
      try {
        saved = await configManager.updateAuthCredentialSettings(body);
      } catch (error) {
        if (sessionIpMobilityChanged) {
          try {
            await authMobilitySessionManager.reconcileSessionIpMobilityPolicy(
              next,
              previous,
              { scheduleSync: false },
            );
            scheduleSyncReverseProxyTrustedIPs({
              reason: "session-ip-mobility-config-rollback",
              delayMs: 0,
            });
          } catch (rollbackError) {
            console.error(
              "[auth-mobility] failed to rollback session IP mobility reconciliation after config save failure:",
              rollbackError,
            );
          }
        }
        throw error;
      }
      if (sessionIpMobilityChanged) {
        scheduleSyncReverseProxyTrustedIPs({
          reason: "session-ip-mobility-config-updated",
          delayMs: 0,
        });
      }
      return { success: true, data: saved };
    },
    withRouteDoc("更新认证凭据配置", {
      body: t.Object({
        session_ttl_seconds: t.Optional(t.Number()),
        remember_me_ttl_seconds: t.Optional(t.Number()),
        post_login_ip_grant_mode: t.Optional(
          t.Union([
            t.Literal("follow_session"),
            t.Literal("disabled"),
            t.Literal("custom"),
          ]),
        ),
        post_login_ip_grant_ttl_seconds: t.Optional(
          t.Union([t.Number(), t.Null()]),
        ),
        session_ip_mobility_enabled: t.Optional(t.Boolean()),
        session_ip_mobility_window_seconds: t.Optional(t.Number()),
        passkey_bind_prompt_enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/totp/status",
    async () => {
      const credentials = await configManager.getTOTPCredentials();
      return {
        success: true,
        data: { bound: credentials.length > 0, credentials },
      };
    },
    routeDoc("获取 TOTP 绑定状态"),
  )
  .post(
    "/totp/setup",
    async () => {
      const secret = generateSecret();
      const uri = generateURI({
        issuer: "fn-knock",
        label: "admin",
        secret,
        strategy: "totp",
      });
      return { success: true, data: { secret, uri } };
    },
    routeDoc("生成 TOTP 绑定信息"),
  )
  .post(
    "/totp/bind",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const { valid } = verifySync({
        strategy: "totp",
        token: body.token,
        secret: body.secret,
      });
      if (!valid) {
        set.status = 400;
        return { success: false, message: adminT(t, "totp.invalidCode") };
      }
      await configManager.addTOTPCredential({
        id: randomBytes(8).toString("hex"),
        secret: body.secret,
        comment: body.comment || "New Token",
        createdAt: new Date().toISOString(),
        access_scopes: [],
        subdomain_access: { mode: "all", hosts: [] },
      });
      return { success: true };
    },
    withRouteDoc("绑定 TOTP 凭据", {
      body: t.Object({
        secret: t.String(),
        token: t.String(),
        comment: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/totp/credentials/export",
    async () => {
      const payload = buildTOTPCredentialTransferPayload({
        credentials: await configManager.getTOTPCredentials(),
        appVersion: APP_LOCAL_VERSION,
      });
      const body = new Blob([JSON.stringify(payload, null, 2)], {
        type: "application/json",
      });

      return new Response(body, {
        headers: {
          "Content-Type": "application/json; charset=utf-8",
          "Content-Disposition": `attachment; filename="${buildTOTPCredentialExportFilename(
            payload.exported_at,
          )}"`,
          "Cache-Control": "no-store",
        },
      });
    },
    routeDoc("导出 TOTP 凭据"),
  )
  .post(
    "/totp/credentials/import",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      try {
        const existing = await configManager.getTOTPCredentials();
        const plan = buildTOTPCredentialImportPlan({
          existing,
          payload: body.payload,
        });

        if (plan.credentials.length > 0) {
          await configManager.saveTOTPCredentials([
            ...existing,
            ...plan.credentials,
          ]);
        }

        return { success: true, data: plan.summary };
      } catch (error) {
        if (error instanceof TOTPCredentialTransferError) {
          set.status = error.status;
          return {
            success: false,
            message: adminT(t, `totpImport.${error.code}`, error.params),
          };
        }
        throw error;
      }
    },
    withRouteDoc("导入 TOTP 凭据", {
      body: t.Object({
        payload: t.Any(),
      }),
    }),
  )
  .delete(
    "/totp/:id",
    async ({ request, params, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const deleted = await configManager.deleteTOTPCredential(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: adminT(t, "totp.notFound") };
      }
      await destroySessionsForTotpCredential(params.id);
      await oidcAuthService.deleteBindingsByTotp(params.id);
      await refreshGatewayAuthRuntime(t);
      return { success: true };
    },
    withRouteDoc("删除 TOTP 凭据", {
      params: t.Object({ id: t.String() }),
    }),
  )
  .patch(
    "/totp/:id/access-scopes",
    async ({ request, params, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const updated = await configManager.updateTOTPCredentialAccessScopes(
        params.id,
        body.access_scopes,
      );
      if (!updated) {
        set.status = 404;
        return { success: false, message: adminT(t, "totp.notFound") };
      }
      return { success: true, data: updated };
    },
    withRouteDoc("更新 TOTP 凭据访问范围", {
      params: t.Object({ id: t.String() }),
      body: t.Object({
        access_scopes: t.Array(t.String()),
      }),
    }),
  )
  .patch(
    "/totp/:id/subdomain-access",
    async ({ request, params, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const updated = await configManager.updateTOTPCredentialSubdomainAccess(
        params.id,
        body.subdomain_access,
      );
      if (!updated) {
        set.status = 404;
        return { success: false, message: adminT(t, "totp.notFound") };
      }
      if (updated.subdomain_access.mode === "custom") {
        await clearAutoIpGrantsForTotpCredential(params.id);
      }
      await refreshGatewayAuthRuntime(t);
      return { success: true, data: updated };
    },
    withRouteDoc("更新 TOTP 凭据子域访问范围", {
      params: t.Object({ id: t.String() }),
      body: t.Object({
        subdomain_access: t.Object({
          mode: t.Union([t.Literal("all"), t.Literal("custom")]),
          hosts: t.Array(t.String()),
        }),
      }),
    }),
  )
  .patch(
    "/totp/:id/comment",
    async ({ request, params, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const updated = await configManager.updateTOTPCredential(
        params.id,
        body.comment,
      );
      if (!updated) {
        set.status = 404;
        return { success: false, message: adminT(t, "totp.notFound") };
      }
      return { success: true };
    },
    withRouteDoc("更新 TOTP 凭据备注", {
      params: t.Object({ id: t.String() }),
      body: t.Object({ comment: t.String() }),
    }),
  )
  .get(
    "/totp/:totpId/passkeys",
    async ({ params }) => {
      const passkeys = await configManager.getPasskeys();
      const filtered = passkeys.filter((pk) => pk.totpId === params.totpId);
      return { success: true, data: filtered };
    },
    withRouteDoc("获取 TOTP 关联的 Passkey 列表", {
      params: t.Object({ totpId: t.String() }),
    }),
  )
  .delete(
    "/passkeys/:id",
    async ({ request, params, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const deleted = await configManager.deletePasskey(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: adminT(t, "passkeys.notFound") };
      }
      return { success: true };
    },
    withRouteDoc("删除 Passkey", {
      params: t.Object({
        id: t.String(),
      }),
    }),
  );
