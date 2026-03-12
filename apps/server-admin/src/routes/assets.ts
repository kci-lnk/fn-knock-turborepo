import { Elysia, t } from "elysia";
import { portScannerPlugin } from "../plugins/scanner";
import { acmePlugin } from "../plugins/acme";
import { ConfigManager } from "../lib/redis";

export const assetsRoutes = new Elysia({ prefix: "/api/admin/scan" })
  .use(portScannerPlugin)
  .use(acmePlugin)
  .get(
    "/discover",
    async ({ scannerService }) => {
      const configManager = new ConfigManager();
      const config = await configManager.getConfig();
      const proxy_mappings = config.proxy_mappings || [];

      const targetIp = "127.0.0.1";
      const envPorts = [
        parseInt(process.env.BACKEND_PORT || "7998", 10),
        parseInt(process.env.AUTH_PORT || "7997", 10),
        parseInt(process.env.GO_BACKEND_PORT || "7996", 10),
        parseInt(process.env.GO_REPROXY_PORT || "7999", 10),
        7995,
        8000 // 旧的飞牛端口
      ];

      const mappingPorts: number[] = [];
      for (const mapping of proxy_mappings) {
        if (mapping.target) {
          try {
            const parsedUrl = new URL(mapping.target);
            if (parsedUrl.port) {
              mappingPorts.push(parseInt(parsedUrl.port, 10));
            } else if (parsedUrl.protocol === "http:") {
              mappingPorts.push(80);
            } else if (parsedUrl.protocol === "https:") {
              mappingPorts.push(443);
            }
          } catch (e) {
            console.warn(`[扫描警告] 无法解析代理映射的 URL: ${mapping.target}`);
          }
        }
      }

      const excludePorts = Array.from(new Set([...envPorts, ...mappingPorts]));
      console.log("准备跳过的端口:", excludePorts);

      try {
        const scanResult = await scannerService.scanAndAnalyze(targetIp, {
          skipPorts: excludePorts,
          maxConcurrent: 200,
        });
        return {
          success: true,
          data: scanResult,
        };
      } catch (error) {
        return {
          success: false,
          message: (error as Error).message,
        };
      }
    }
  );