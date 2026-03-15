import { Elysia, t } from "elysia";
import { ddnsLogBuffer, ddnsManager } from "../lib/ddns";
import { runAutomaticDDNSCheck } from "../lib/ddns/auto-check";
import { applyUpdateScope, getUpdateScopeDetectionOptions, getUpdateScopeUnavailableMessage } from "../lib/ddns/providers/helpers";
import { IPDetector } from "../plugins/ip-detector";

const parseDDNSLogEntries = (raw: string[]) =>
  raw.map((s) => {
    try { return JSON.parse(s); } catch { return { time: "", level: "info", message: s }; }
  });

export const ddnsRoutes = new Elysia({ prefix: "/api/admin/ddns" })

  // ── Status ────────────────────────────────────────────────────
  .get("/status", async () => {
    const status = await ddnsManager.getStatus();
    return { success: true, data: status };
  })

  // ── Toggle ────────────────────────────────────────────────────
  .post("/toggle", async ({ body }) => {
    const wasEnabled = await ddnsManager.isEnabled();
    await ddnsManager.setEnabled(body.enabled);

    if (body.enabled && !wasEnabled) {
      void runAutomaticDDNSCheck({
        trigger: "enable",
        emitSkipLog: true,
        emitNoopLog: true,
      });
    }

    return { success: true };
  }, {
    body: t.Object({ enabled: t.Boolean() }),
  })

  // ── Providers list ────────────────────────────────────────────
  .get("/providers", () => {
    const providers = ddnsManager.getProviders();
    return { success: true, data: providers };
  })

  .get("/interfaces", () => {
    const interfaces = ddnsManager.listNetworkInterfaces();
    return { success: true, data: interfaces };
  })

  // ── Set current provider ──────────────────────────────────────
  .post("/provider", async ({ body, set }) => {
    try {
      await ddnsManager.setProvider(body.provider);
      return { success: true };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || "设置提供商失败" };
    }
  }, {
    body: t.Object({ provider: t.String() }),
  })

  // ── Get config for provider ───────────────────────────────────
  .get("/config/:provider", async ({ params }) => {
    const config = await ddnsManager.getConfig(params.provider);
    return { success: true, data: config };
  })

  // ── Save config for provider ──────────────────────────────────
  .post("/config/:provider", async ({ params, body }) => {
    await ddnsManager.saveConfig(params.provider, body.config);
    return { success: true };
  }, {
    body: t.Object({ config: t.Record(t.String(), t.String()) }),
  })

  // ── Test (manual trigger) ─────────────────────────────────────
  .post("/test", async ({ set }) => {
    try {
      const provider = await ddnsManager.getProvider();
      if (!provider) {
        set.status = 400;
        return { success: false, message: "请先选择 DDNS 提供商" };
      }

      const complete = await ddnsManager.isConfigComplete();
      if (!complete) {
        set.status = 400;
        return { success: false, message: "当前提供商配置不完整，请填写所有必填字段" };
      }

      await ddnsManager.appendLog("info", "手动测试开始，正在获取当前 IP...");

      const updateScope = await ddnsManager.getUpdateScope(provider);
      const networkInterface = await ddnsManager.getNetworkInterface(provider);
      const detectionOptions = getUpdateScopeDetectionOptions(updateScope);
      const ips = await IPDetector.getCurrentIPs({ networkInterface, ...detectionOptions });
      await ddnsManager.appendLog("info", `检测到 IP — IPv4: ${ips.ipv4 || "无"}, IPv6: ${ips.ipv6 || "无"}`);
      if (detectionOptions.enableIPv4 && ips.errors.ipv4 && ips.ipv6) {
        await ddnsManager.appendLog("warn", `IPv4 获取失败，将继续使用 IPv6 (${ips.errors.ipv4})`);
      }
      if (detectionOptions.enableIPv6 && ips.errors.ipv6 && ips.ipv4) {
        await ddnsManager.appendLog("warn", `IPv6 获取失败，将继续使用 IPv4 (${ips.errors.ipv6})`);
      }

      if (!ips.ipv4 && !ips.ipv6) {
        await ddnsManager.appendLog("error", "无法获取任何公网 IP，测试中止");
        set.status = 500;
        return { success: false, message: "无法获取公网 IP" };
      }

      const scopedIPs = applyUpdateScope(updateScope, ips.ipv4, ips.ipv6);
      if (!scopedIPs.ipv4 && !scopedIPs.ipv6) {
        const message = getUpdateScopeUnavailableMessage(updateScope);
        await ddnsManager.appendLog("error", `${message}，测试中止`);
        set.status = 400;
        return { success: false, message };
      }

      const result = await ddnsManager.executeUpdate(ips.ipv4, ips.ipv6);

      if (result.success) {
        await ddnsManager.setLastIP(scopedIPs.ipv4, scopedIPs.ipv6, { merge: true });
        await ddnsManager.appendLog("info", `更新成功: ${result.message}`);
      } else {
        await ddnsManager.appendLog("error", `更新失败: ${result.message}`);
      }

      return { success: result.success, message: result.message, data: { ipv4: ips.ipv4, ipv6: ips.ipv6 } };
    } catch (e: any) {
      const msg = e?.message || String(e);
      await ddnsManager.appendLog("error", `测试异常: ${msg}`);
      set.status = 500;
      return { success: false, message: msg };
    }
  })

  // ── Logs ──────────────────────────────────────────────────────
  .get("/logs", async ({ query }) => {
    const limit = Math.max(1, Math.min(parseInt((query.limit as any) || "200", 10), 1000));
    const logs = await ddnsManager.getLogs(limit);
    return { success: true, data: logs };
  })

  .delete("/logs", async () => {
    await ddnsManager.clearLogs();
    return { success: true };
  })

  // ── Polling ──────────────────────────────────────────────────
  .get("/poll", async ({ query }) => {
    const { cursor, reset, items } = await ddnsLogBuffer.poll(query.cursor);
    const status = await ddnsManager.getStatus();

    return {
      success: true,
      data: {
        cursor,
        reset,
        logs: parseDDNSLogEntries(items),
        status,
      },
    };
  }, {
    query: t.Object({
      cursor: t.Optional(t.String()),
    }),
  });
