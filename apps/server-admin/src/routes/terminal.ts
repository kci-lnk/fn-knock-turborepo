import { Elysia, t } from "elysia";
import { terminalManager } from "../lib/terminal-manager";

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

export const terminalRoutes = new Elysia({ prefix: "/api/admin/terminal" })
  .get("/status", async () => {
    const status = await terminalManager.getRuntimeStatus();
    return { success: true, data: status };
  })
  .get("/sessions", async () => {
    const sessions = await terminalManager.listSessions();
    return { success: true, data: sessions };
  })
  .get("/sessions/:id", async ({ params, set }) => {
    const session = await terminalManager.getSession(params.id);
    if (!session) {
      set.status = 404;
      return { success: false, message: "终端会话不存在" };
    }
    return { success: true, data: session };
  })
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
    {
      body: t.Object({
        title: t.Optional(t.String()),
        shell: t.Optional(t.String()),
        cwd: t.Optional(t.String()),
        cols: t.Optional(t.Number()),
        rows: t.Optional(t.Number()),
      }),
    },
  )
  .patch(
    "/sessions/:id",
    async ({ params, body, set }) => {
      const session = await terminalManager.renameSession(
        params.id,
        body.title,
      );
      if (!session) {
        set.status = 404;
        return { success: false, message: "终端会话不存在" };
      }
      return { success: true, data: session };
    },
    {
      body: t.Object({
        title: t.String(),
      }),
    },
  )
  .delete("/sessions/:id", async ({ params }) => {
    await terminalManager.killSession(params.id);
    return { success: true };
  })
  .post("/sessions/:id/attachments", async ({ params, request }) => {
    const attachment = await terminalManager.createAttachment(
      params.id,
      detectClientIp(request),
    );
    return { success: true, data: attachment };
  })
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
    {
      query: t.Object({
        cursor: t.Optional(t.Numeric()),
        timeout_ms: t.Optional(t.Number()),
      }),
    },
  )
  .post(
    "/attachments/:id/input",
    async ({ params, body }) => {
      await terminalManager.sendInput(params.id, body.dataBase64);
      return { success: true };
    },
    {
      body: t.Object({
        dataBase64: t.String(),
      }),
    },
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
    {
      body: t.Object({
        cols: t.Number(),
        rows: t.Number(),
      }),
    },
  )
  .delete("/attachments/:id", async ({ params }) => {
    await terminalManager.detachAttachment(params.id);
    return { success: true };
  });
