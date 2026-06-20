import { Elysia, t } from "elysia";
import { authMobilitySessionManager } from "../../lib/auth-mobility-session";
import { ipLocationRefs, ipLocationService } from "../../lib/ip-location";
import {
  normalizeAutoIpGrantComment,
  revokeCustomPostLoginIpGrant,
} from "../../lib/post-login-ip-grant";
import { type LoginSession, configManager } from "../../lib/redis";
import { scheduleSyncReverseProxyTrustedIPs } from "../../lib/reverse-proxy-trusted-ips";
import { emitLogoutEvent } from "../../lib/system-events/helpers";
import { whitelistManager } from "../../lib/whitelist-manager";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import { adminT, getAdminRouteTranslator } from "./shared";

const resolveSessionDefaultComment = async (
  sessionId: string,
  session: LoginSession,
  locale?: string | null,
): Promise<string | undefined> => {
  const sessionGrantRecord = session.postLoginIpGrantRecordId
    ? await whitelistManager.getRecordById(session.postLoginIpGrantRecordId)
    : null;
  if (
    sessionGrantRecord?.status === "active" &&
    sessionGrantRecord.comment !== undefined
  ) {
    return normalizeAutoIpGrantComment(sessionGrantRecord.comment, locale);
  }

  const [whitelistRecordId] =
    await authMobilitySessionManager.listSessionWhitelistRecordIds(sessionId);
  const boundRecord = whitelistRecordId
    ? await whitelistManager.getRecordById(whitelistRecordId)
    : null;
  if (boundRecord?.status === "active" && boundRecord.comment !== undefined) {
    return normalizeAutoIpGrantComment(boundRecord.comment, locale);
  }

  const latestRecord = await whitelistManager.getLatestActiveRecordByIP(
    session.ip,
  );
  if (!latestRecord || latestRecord.comment === undefined) {
    return undefined;
  }

  return normalizeAutoIpGrantComment(latestRecord.comment, locale);
};

const ensureSessionComment = async (
  sessionId: string,
  session: LoginSession,
  locale?: string | null,
): Promise<LoginSession> => {
  if (session.comment !== undefined) {
    const comment = normalizeAutoIpGrantComment(session.comment, locale);
    if (comment === session.comment) return session;
    return {
      ...session,
      comment,
    };
  }

  const comment = await resolveSessionDefaultComment(
    sessionId,
    session,
    locale,
  );
  if (comment === undefined) {
    return session;
  }

  return (
    (await configManager.updateSession(sessionId, { comment })) ?? {
      ...session,
      comment,
    }
  );
};

export const adminSessionRoutes = new Elysia()
  .get(
    "/sessions",
    async ({ request }) => {
      const { locale } = await getAdminRouteTranslator(request);
      const list = await configManager.listSessions();
      const mapped = await Promise.all(
        list.map(async ({ id, data }) => {
          const session = await ensureSessionComment(id, data, locale);
          const [mobility, fnosAttachments, trimMediaAttachments] =
            await Promise.all([
              authMobilitySessionManager.getSessionMobilitySummary(id),
              authMobilitySessionManager.listSessionFnosAttachments(id),
              authMobilitySessionManager.listSessionTrimMediaAttachments(id),
            ]);
          return {
            id,
            ...session,
            mobility,
            fnosAttachments,
            trimMediaAttachments,
          };
        }),
      );
      await ipLocationService.hydrateIpLocationRecords(mapped, (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: mapped };
    },
    routeDoc("获取会话列表"),
  )
  .get(
    "/sessions/:id",
    async ({ request, params, set }) => {
      const { locale, t } = await getAdminRouteTranslator(request);
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: adminT(t, "sessions.notFound") };
      }
      const session = await ensureSessionComment(params.id, sess, locale);
      const [mobility, fnosAttachments, trimMediaAttachments] =
        await Promise.all([
          authMobilitySessionManager.getSessionMobilitySummary(params.id),
          authMobilitySessionManager.listSessionFnosAttachments(params.id),
          authMobilitySessionManager.listSessionTrimMediaAttachments(params.id),
        ]);
      const record = {
        id: params.id,
        ...session,
        mobility,
        fnosAttachments,
        trimMediaAttachments,
      };
      await ipLocationService.hydrateIpLocationRecords([record], (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: record };
    },
    withRouteDoc("获取会话详情", {
      params: t.Object({ id: t.String() }),
    }),
  )
  .patch(
    "/sessions/:id/comment",
    async ({ request, params, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: adminT(t, "sessions.notFound") };
      }

      const updated = await configManager.updateSession(params.id, {
        comment: body.comment,
      });
      if (!updated) {
        set.status = 404;
        return { success: false, message: adminT(t, "sessions.notFound") };
      }

      const whitelistRecordIds = new Set<string>();
      if (updated.postLoginIpGrantRecordId) {
        whitelistRecordIds.add(updated.postLoginIpGrantRecordId);
      }

      for (const mobilityWhitelistRecordId of await authMobilitySessionManager.listSessionWhitelistRecordIds(
        params.id,
      )) {
        whitelistRecordIds.add(mobilityWhitelistRecordId);
      }

      for (const whitelistRecordId of whitelistRecordIds) {
        await whitelistManager.updateComment(whitelistRecordId, body.comment);
      }

      const record = { id: params.id, ...updated };
      await ipLocationService.hydrateIpLocationRecords([record], (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: record };
    },
    withRouteDoc("更新会话备注", {
      params: t.Object({ id: t.String() }),
      body: t.Object({ comment: t.String() }),
    }),
  )
  .get(
    "/sessions/:id/mobility",
    async ({ request, params, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: adminT(t, "sessions.notFound") };
      }
      const details =
        await authMobilitySessionManager.getSessionMobilityDetails(params.id);
      await ipLocationService.hydrateMobilityEvents(details.events, params.id);
      return {
        success: true,
        data: details,
      };
    },
    withRouteDoc("获取会话漫游详情", {
      params: t.Object({ id: t.String() }),
    }),
  )
  .delete(
    "/sessions/:id",
    async ({ params }) => {
      const sess = await configManager.getSession(params.id);
      if (sess) {
        const config = await configManager.getConfig();
        const sessionComment = normalizeAutoIpGrantComment(
          sess.comment,
          config.locale?.default_locale,
        );
        await authMobilitySessionManager.destroySession(params.id);
        await configManager.deleteSession(params.id);
        await revokeCustomPostLoginIpGrant(sess, config, sess.ip);
        scheduleSyncReverseProxyTrustedIPs({
          reason: "admin-session-delete",
        });
        await emitLogoutEvent({
          sessionId: params.id,
          authMethod: sess.method,
          credentialId: sess.credentialId,
          credentialName: sess.credentialName,
          ...(sess.linkedTotpName
            ? { linkedTotpName: sess.linkedTotpName }
            : {}),
          ...(sessionComment ? { sessionComment } : {}),
          ip: sess.ip,
          ...(sess.ipLocation ? { ipLocation: sess.ipLocation } : {}),
          userAgent: sess.userAgent,
          ...(sess.loginTime ? { loginTime: sess.loginTime } : {}),
          logoutSource: "admin_session_delete",
        });
      } else {
        await configManager.deleteSession(params.id);
      }
      return { success: true };
    },
    withRouteDoc("强制注销会话", {
      params: t.Object({ id: t.String() }),
    }),
  );
