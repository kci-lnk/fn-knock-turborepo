import { Elysia, t } from "elysia";
import { whitelistManager } from "../lib/whitelist-manager";

export const whitelistRoutes = new Elysia({ prefix: "/api/admin/whitelist" })
  .get("/", async () => {
    const records = await whitelistManager.getAllActiveRecords();
    return { success: true, data: records };
  })
  .post(
    "/",
    async ({ body }) => {
      const id = await whitelistManager.addWhiteList({
        ip: body.ip,
        expireAt: body.expireAt,
        source: body.source,
        comment: body.comment,
      });
      return { success: true, data: { id } };
    },
    {
      body: t.Object({
        ip: t.String(),
        expireAt: t.Union([t.Number(), t.Null()]),
        source: t.Union([t.Literal("manual"), t.Literal("auto")]),
        comment: t.Optional(t.String()),
      }),
    },
  )
  .delete(
    "/:id",
    async ({ params, set }) => {
      const deleted = await whitelistManager.removeWhiteList(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: "Record not found" };
      }
      return { success: true };
    },
    {
      params: t.Object({
        id: t.String(),
      }),
    },
  )
  .patch(
    "/:id/comment",
    async ({ params, body, set }) => {
      const updated = await whitelistManager.updateComment(
        params.id,
        body.comment,
      );
      if (!updated) {
        set.status = 404;
        return { success: false, message: "Record not found" };
      }
      return { success: true };
    },
    {
      params: t.Object({
        id: t.String(),
      }),
      body: t.Object({
        comment: t.String(),
      }),
    },
  );
