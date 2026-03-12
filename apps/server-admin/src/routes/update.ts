import { Elysia } from "elysia";
import { updateManager } from "../lib/update-manager";

export const updateRoutes = new Elysia({ prefix: "/api/admin/update" })
  .get("/status", async () => {
    const data = await updateManager.getStatus();
    return { success: true, data };
  })
  .post("/check", async () => {
    await updateManager.checkNow("manual");
    const data = await updateManager.getStatus();
    return { success: true, data };
  })
  .post("/download", async ({ set }) => {
    try {
      await updateManager.triggerDownload();
      const data = await updateManager.getStatus();
      return { success: true, message: "已开始下载更新包", data };
    } catch (error) {
      set.status = 400;
      return { success: false, message: error instanceof Error ? error.message : "启动下载失败" };
    }
  })
  .post("/install", async ({ set }) => {
    try {
      await updateManager.triggerInstall();
      return { success: true, message: "更新安装流程已启动" };
    } catch (error) {
      set.status = 400;
      return { success: false, message: error instanceof Error ? error.message : "启动安装失败" };
    }
  })
  .post("/check-and-download", async ({ set }) => {
    try {
      await updateManager.checkNow("manual-check-and-download");
      await updateManager.triggerDownload();
      const data = await updateManager.getStatus();
      return { success: true, message: "已发起检查并开始下载", data };
    } catch (error) {
      set.status = 400;
      return { success: false, message: error instanceof Error ? error.message : "启动失败" };
    }
  })
  .get("/confirm", async () => {
    const data = await updateManager.consumeConfirmMessage();
    return { success: true, data };
  });
