import { Elysia } from "elysia";
import { frpManager } from "../lib/frp-manager";
import { cloudflaredManager } from "../lib/cloudflared-manager";
import { configManager } from "../lib/redis";
import { resolveAccessEntryInfo } from "../lib/access-entry";

export const systemRoutes = new Elysia({ prefix: "/api/admin/system" })
    .get("/access-entry", async () => {
        const config = await configManager.getConfig();
        return {
            success: true,
            data: resolveAccessEntryInfo(config),
        };
    })
    .get("/cloudflared/status", () => {
        return { success: true, data: cloudflaredManager.getStatus() };
    })
    .post("/cloudflared/download", async () => {
        cloudflaredManager.startDownload();
        return { success: true, message: "Download started" };
    })
    .post("/cloudflared/cancel", () => {
        cloudflaredManager.cancelDownload();
        return { success: true, message: "Download cancelled" };
    })
    .delete("/cloudflared", async () => {
        await cloudflaredManager.delete();
        return { success: true, message: "Deleted" };
    })
    .get("/frp/status", () => {
        return { success: true, data: frpManager.getStatus() };
    })
    .post("/frp/download", async () => {
        frpManager.startDownload();
        return { success: true, message: "Download started" };
    })
    .post("/frp/cancel", () => {
        frpManager.cancelDownload();
        return { success: true, message: "Download cancelled" };
    })
    .delete("/frp", async () => {
        await frpManager.delete();
        return { success: true, message: "Deleted" };
    });
