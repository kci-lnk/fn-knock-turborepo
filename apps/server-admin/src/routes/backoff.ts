import { Elysia, t } from "elysia";
import { loginBackoffService } from "../lib/login-backoff";

export const backoffRoutes = new Elysia({ prefix: "/api/admin/backoff" })
  .get("/list", async () => {
    const items = await loginBackoffService.listBlocked();
    return { success: true, data: items };
  })
  .get("/status", async ({ query, set }) => {
    const ip = query.ip as string | undefined;
    if (!ip) {
      set.status = 400;
      return { success: false, message: "ip 参数缺失" };
    }
    const st = await loginBackoffService.getStatus(ip);
    return { success: true, data: st };
  })
  .post("/reset", async ({ body }) => {
    await loginBackoffService.reset(body.ip);
    return { success: true };
  }, {
    body: t.Object({
      ip: t.String()
    })
  });

