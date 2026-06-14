import { Elysia, t } from "elysia";
import { terminalManager } from "../lib/terminal-manager";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
} from "../lib/runtime-profile";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const getTerminalRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

const detectClientIp = (request: Request): string => {
  const forwarded = request.headers.get("x-forwarded-for");
  if (forwarded) {
    const first = forwarded.split(",")[0]?.trim();
    if (first) return first;
  }
  const realIp = request.headers.get("x-real-ip")?.trim();
  if (realIp) return realIp;
  return "unknown";
};

export const terminalRoutes = new Elysia({
  prefix: "/api/admin/terminal",
  tags: ["Terminal"],
})
  .onBeforeHandle(async ({ set, request }) => {
    if (getRuntimeCapabilities().terminal_available) {
      return;
    }

    const { locale } = await getTerminalRouteTranslator(request);
    set.status = 403;
    return {
      success: false,
      message: getCapabilityUnavailableMessage(
        "terminal_available",
        undefined,
        locale,
      ),
    };
  })
  .get(
    "/status",
    async () => {
      const status = await terminalManager.getRuntimeStatus();
      return { success: true, data: status };
    },
    routeDoc("获取终端功能状态"),
  )
  .post(
    "/tmux/install",
    async () => {
      const state = await terminalManager.startTmuxInstall();
      return { success: true, data: state };
    },
    routeDoc("安装 tmux"),
  )
  .get(
    "/sessions",
    async () => {
      const sessions = await terminalManager.listSessions();
      return { success: true, data: sessions };
    },
    routeDoc("获取终端会话列表"),
  )
  .get(
    "/sessions/:id",
    async ({ params, set, request }) => {
      const { t } = await getTerminalRouteTranslator(request);
      const session = await terminalManager.getSession(params.id);
      if (!session) {
        set.status = 404;
        return {
          success: false,
          message: t("server.terminal.sessionNotFound"),
        };
      }
      return { success: true, data: session };
    },
    routeDoc("获取终端会话详情"),
  )
  .post(
    "/sessions",
    async ({ body, request }) => {
      const session = await terminalManager.createSession(
        {
          title: body.title,
          shell: body.shell,
          cwd: body.cwd,
          cols: body.cols,
          rows: body.rows,
        },
        detectClientIp(request),
      );
      return { success: true, data: session };
    },
    withRouteDoc("创建终端会话", {
      body: t.Object({
        title: t.Optional(t.String()),
        shell: t.Optional(t.String()),
        cwd: t.Optional(t.String()),
        cols: t.Optional(t.Number()),
        rows: t.Optional(t.Number()),
      }),
    }),
  )
  .patch(
    "/sessions/:id",
    async ({ params, body, set, request }) => {
      const { t } = await getTerminalRouteTranslator(request);
      const session = await terminalManager.renameSession(
        params.id,
        body.title,
      );
      if (!session) {
        set.status = 404;
        return {
          success: false,
          message: t("server.terminal.sessionNotFound"),
        };
      }
      return { success: true, data: session };
    },
    withRouteDoc("重命名终端会话", {
      body: t.Object({
        title: t.String(),
      }),
    }),
  )
  .delete(
    "/sessions/:id",
    async ({ params }) => {
      await terminalManager.killSession(params.id);
      return { success: true };
    },
    routeDoc("关闭终端会话"),
  )
  .post(
    "/sessions/:id/attachments",
    async ({ params, request }) => {
      const attachment = await terminalManager.createAttachment(
        params.id,
        detectClientIp(request),
      );
      return { success: true, data: attachment };
    },
    routeDoc("创建终端附件连接"),
  )
  .get(
    "/attachments/:id/poll",
    async ({ params, query }) => {
      const result = await terminalManager.waitForOutput(
        params.id,
        query.cursor,
        query.timeout_ms || undefined,
      );
      return { success: true, data: result };
    },
    withRouteDoc("轮询终端输出", {
      query: t.Object({
        cursor: t.Optional(t.Numeric()),
        timeout_ms: t.Optional(t.Number()),
      }),
    }),
  )
  .post(
    "/attachments/:id/input",
    async ({ params, body }) => {
      await terminalManager.sendInput(params.id, body.dataBase64);
      return { success: true };
    },
    withRouteDoc("向终端发送输入", {
      body: t.Object({
        dataBase64: t.String(),
      }),
    }),
  )
  .post(
    "/attachments/:id/resize",
    async ({ params, body }) => {
      const session = await terminalManager.resizeAttachment(
        params.id,
        body.cols,
        body.rows,
      );
      return { success: true, data: session };
    },
    withRouteDoc("调整终端窗口大小", {
      body: t.Object({
        cols: t.Number(),
        rows: t.Number(),
      }),
    }),
  )
  .delete(
    "/attachments/:id",
    async ({ params }) => {
      await terminalManager.detachAttachment(params.id);
      return { success: true };
    },
    routeDoc("关闭终端附件连接"),
  );
