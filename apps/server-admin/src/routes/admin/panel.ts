import { Elysia, t } from "elysia";
import { getClientIp } from "../../lib/auth-request";
import { dockerAdminPanelManager } from "../../lib/docker-admin-panel";
import {
  buildAdminPanelSessionClearCookie,
  buildAdminPanelSessionCookie,
} from "../../lib/session-cookie";
import { configManager } from "../../lib/redis";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import { adminT, getAdminRouteTranslator, isPanelAuthRuntime } from "./shared";

export const adminPanelRoutes = new Elysia()
  .get(
    "/panel/bootstrap",
    async ({ request }) => {
      const locale = await configManager.getLocaleConfig();
      return {
        success: true,
        data: await dockerAdminPanelManager.buildBootstrapState(
          request,
          isPanelAuthRuntime(),
          locale,
        ),
      };
    },
    routeDoc("获取 Docker 管理面板登录状态"),
  )
  .post(
    "/panel/password",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      if (!isPanelAuthRuntime()) {
        set.status = 400;
        return {
          success: false,
          message: adminT(t, "dockerPanel.passwordNotNeeded"),
        };
      }

      try {
        await dockerAdminPanelManager.setPassword(body.password);
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : adminT(t, "dockerPanel.setPasswordFailed"),
        };
      }

      const session = await dockerAdminPanelManager.createSession({
        ip: getClientIp(request) || "unknown",
        userAgent: request.headers.get("user-agent") || "",
      });
      const locale = await configManager.getLocaleConfig();
      await dockerAdminPanelManager.resetLoginFailures(getClientIp(request));
      set.headers["set-cookie"] = buildAdminPanelSessionCookie(
        session.id,
        dockerAdminPanelManager.sessionTtlSeconds,
        {
          secure: dockerAdminPanelManager.isSecureRequest(request),
        },
      );

      return {
        success: true,
        data: {
          enabled: true,
          password_configured: true,
          authenticated: true,
          auth_source: "panel_session",
          session_expires_at: session.expires_at,
          locale,
        },
      };
    },
    withRouteDoc("首次设置 Docker 管理面板密码", {
      body: t.Object({
        password: t.String(),
      }),
    }),
  )
  .post(
    "/panel/password/change",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      if (!isPanelAuthRuntime()) {
        set.status = 400;
        return {
          success: false,
          message: adminT(t, "dockerPanel.passwordChangeUnsupported"),
        };
      }

      try {
        await dockerAdminPanelManager.changePassword(body.password);
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : adminT(t, "dockerPanel.changePasswordFailed"),
        };
      }

      const session = await dockerAdminPanelManager.createSession({
        ip: getClientIp(request) || "unknown",
        userAgent: request.headers.get("user-agent") || "",
      });
      const locale = await configManager.getLocaleConfig();
      set.headers["set-cookie"] = buildAdminPanelSessionCookie(
        session.id,
        dockerAdminPanelManager.sessionTtlSeconds,
        {
          secure: dockerAdminPanelManager.isSecureRequest(request),
        },
      );

      return {
        success: true,
        data: {
          enabled: true,
          password_configured: true,
          authenticated: true,
          auth_source: "panel_session",
          session_expires_at: session.expires_at,
          locale,
        },
      };
    },
    withRouteDoc("修改 Docker 管理面板密码", {
      body: t.Object({
        password: t.String(),
      }),
    }),
  )
  .post(
    "/panel/login",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const locale = await configManager.getLocaleConfig();
      if (!isPanelAuthRuntime()) {
        return {
          success: true,
          data: await dockerAdminPanelManager.buildBootstrapState(
            request,
            false,
            locale,
          ),
        };
      }

      const clientIp = getClientIp(request) || "unknown";
      const gate = await dockerAdminPanelManager.ensureLoginAllowed(clientIp);
      if (!gate.allowed) {
        set.status = 429;
        if (gate.retryAfter) {
          set.headers["Retry-After"] = String(gate.retryAfter);
        }
        return {
          success: false,
          message: gate.retryAfter
            ? adminT(t, "dockerPanel.tooManyAttemptsWithRetry", {
                seconds: gate.retryAfter,
              })
            : adminT(t, "dockerPanel.tooManyAttempts"),
          retryAfter: gate.retryAfter,
          blockedUntil: gate.blockedUntil,
        };
      }

      const passwordConfigured =
        await dockerAdminPanelManager.isPasswordConfigured();
      if (!passwordConfigured) {
        set.status = 409;
        return {
          success: false,
          message: adminT(t, "dockerPanel.passwordSetupRequired"),
        };
      }

      const passwordValid = await dockerAdminPanelManager.verifyPassword(
        body.password,
      );
      if (!passwordValid) {
        const failure =
          await dockerAdminPanelManager.registerLoginFailure(clientIp);
        set.status = 429;
        set.headers["Retry-After"] = String(failure.retryAfter);
        return {
          success: false,
          message: adminT(t, "dockerPanel.passwordIncorrectWithRetry", {
            seconds: failure.retryAfter,
          }),
          retryAfter: failure.retryAfter,
          blockedUntil: failure.blockedUntil,
        };
      }

      await dockerAdminPanelManager.resetLoginFailures(clientIp);
      const sessionTtlSeconds =
        body.rememberMe === true
          ? dockerAdminPanelManager.rememberMeSessionTtlSeconds
          : dockerAdminPanelManager.sessionTtlSeconds;
      const session = await dockerAdminPanelManager.createSession({
        ip: clientIp,
        userAgent: request.headers.get("user-agent") || "",
        ttlSeconds: sessionTtlSeconds,
      });
      set.headers["set-cookie"] = buildAdminPanelSessionCookie(
        session.id,
        sessionTtlSeconds,
        {
          secure: dockerAdminPanelManager.isSecureRequest(request),
        },
      );

      return {
        success: true,
        data: {
          enabled: true,
          password_configured: true,
          authenticated: true,
          auth_source: "panel_session",
          session_expires_at: session.expires_at,
          locale,
        },
      };
    },
    withRouteDoc("登录 Docker 管理面板", {
      body: t.Object({
        password: t.String(),
        rememberMe: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/panel/logout",
    async ({ request, set }) => {
      await dockerAdminPanelManager.deleteSessionFromRequest(request);
      set.headers["set-cookie"] = buildAdminPanelSessionClearCookie({
        secure: dockerAdminPanelManager.isSecureRequest(request),
      });

      return {
        success: true,
        data: await dockerAdminPanelManager.buildBootstrapState(
          request,
          isPanelAuthRuntime(),
          await configManager.getLocaleConfig(),
        ),
      };
    },
    routeDoc("退出 Docker 管理面板"),
  );
