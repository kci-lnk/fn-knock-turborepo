import { Elysia } from "elysia";
import { frpManager } from "../lib/frp-manager";
import { cloudflaredManager } from "../lib/cloudflared-manager";

export const systemRoutes = new Elysia({ prefix: "/api/admin/system" })
    .get("/access-entry", () => {
        const goReproxyPort = process.env.GO_REPROXY_PORT || "7999";
        return {
            success: true,
            data: {
                env: "GO_REPROXY_PORT",
                port: goReproxyPort,
                isDefault: !process.env.GO_REPROXY_PORT,
            },
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
