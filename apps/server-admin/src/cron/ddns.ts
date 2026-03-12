import type { Elysia } from "elysia";
import { cron } from "@elysiajs/cron";
import { ddnsManager } from "../lib/ddns";
import { IPDetector } from "../plugins/ip-detector";
import { configManager } from "../lib/redis";

export const registerDDNSCron = (app: Elysia) => {
  const pattern = process.env.DDNS_CRON || "*/5 * * * *";
  const lockTtlSeconds = 120; // 2 minutes lock

  app.use(
    cron({
      name: "ddns-update",
      pattern,
      async run() {
        try {
          const enabled = await ddnsManager.isEnabled();
          if (!enabled) {
            return;
          }

          const provider = await ddnsManager.getProvider();
          if (!provider) {
            return;
          }

          const complete = await ddnsManager.isConfigComplete();
          if (!complete) {
            return;
          }

          const acquired = await configManager.setLockIfNotExists("ddns-update", lockTtlSeconds);
          if (!acquired) {
            return;
          }

          const ips = await IPDetector.getCurrentIPs();
          if (!ips.ipv4 && !ips.ipv6) {
            await ddnsManager.appendLog("warn", "定时检查: 无法获取公网 IP，跳过");
            return;
          }

          const lastIP = await ddnsManager.getLastIP();
          const ipv4Changed = ips.ipv4 !== lastIP.ipv4;
          const ipv6Changed = ips.ipv6 !== lastIP.ipv6;

          if (!ipv4Changed && !ipv6Changed) {
            return;
          }

          const changes: string[] = [];
          if (ipv4Changed) changes.push(`IPv4: ${lastIP.ipv4 || "无"} → ${ips.ipv4 || "无"}`);
          if (ipv6Changed) changes.push(`IPv6: ${lastIP.ipv6 || "无"} → ${ips.ipv6 || "无"}`);
          await ddnsManager.appendLog("info", `检测到 IP 变化: ${changes.join(", ")}`);

          const result = await ddnsManager.executeUpdate(ips.ipv4, ips.ipv6);

          if (result.success) {
            await ddnsManager.setLastIP(ips.ipv4, ips.ipv6);
            await ddnsManager.appendLog("info", `DNS 更新成功 [${provider}]: ${result.message}`);
          } else {
            await ddnsManager.appendLog("error", `DNS 更新失败 [${provider}]: ${result.message}`);
          }
        } catch (e: any) {
          console.error("[ddns][cron] error:", e?.message || String(e));
          await ddnsManager.appendLog("error", `定时任务异常: ${e?.message || String(e)}`);
        }
      },
    })
  );

  return app;
};
