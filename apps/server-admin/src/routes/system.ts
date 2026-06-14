import { Elysia } from "elysia";
import { frpManager } from "../lib/frp-manager";
import { cloudflaredManager } from "../lib/cloudflared-manager";
import { configManager } from "../lib/redis";
import { resolveAccessEntryInfo } from "../lib/access-entry";
import { dnsmasqManager } from "../lib/dnsmasq-manager";
import { systemClockManager } from "../lib/system-clock-manager";
import { routeDoc } from "../lib/openapi";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
} from "../lib/runtime-profile";
import { createRequestTranslator } from "../lib/i18n";

const getSystemRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

export const systemRoutes = new Elysia({
  prefix: "/api/admin/system",
  tags: ["System"],
})
  .get(
    "/clock/status",
    async ({ request }) => {
      const { locale } = await getSystemRouteTranslator(request);
      return { success: true, data: systemClockManager.getStatus(locale) };
    },
    routeDoc("获取系统时间同步状态"),
  )
  .post(
    "/clock/check",
    async ({ request }) => {
      const { locale } = await getSystemRouteTranslator(request);
      const data = await systemClockManager.checkNow(locale);
      return { success: true, data };
    },
    routeDoc("立即检查系统时间"),
  )
  .post(
    "/clock/sync",
    async ({ set, request }) => {
      const { locale, t } = await getSystemRouteTranslator(request);

      if (!getRuntimeCapabilities().system_clock_sync_available) {
        set.status = 403;
        return {
          success: false,
          message: getCapabilityUnavailableMessage(
            "system_clock_sync_available",
            undefined,
            locale,
          ),
        };
      }

      try {
        const result = await systemClockManager.syncNow(locale);
        return { success: true, message: result.message, data: result.data };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.systemClock.syncFailed"),
        };
      }
    },
    routeDoc("立即同步系统时间"),
  )
  .get(
    "/access-entry",
    async () => {
      const config = await configManager.getConfig();
      return {
        success: true,
        data: resolveAccessEntryInfo(config),
      };
    },
    routeDoc("获取当前访问入口信息"),
  )
  .get(
    "/cloudflared/status",
    () => {
      return { success: true, data: cloudflaredManager.getStatus() };
    },
    routeDoc("获取 Cloudflared 安装状态"),
  )
  .post(
    "/cloudflared/download",
    async () => {
      cloudflaredManager.startDownload();
      return { success: true, message: "Download started" };
    },
    routeDoc("下载 Cloudflared"),
  )
  .post(
    "/cloudflared/cancel",
    () => {
      cloudflaredManager.cancelDownload();
      return { success: true, message: "Download cancelled" };
    },
    routeDoc("取消下载 Cloudflared"),
  )
  .delete(
    "/cloudflared",
    async () => {
      await cloudflaredManager.delete();
      return { success: true, message: "Deleted" };
    },
    routeDoc("删除 Cloudflared"),
  )
  .get(
    "/frp/status",
    () => {
      return { success: true, data: frpManager.getStatus() };
    },
    routeDoc("获取 FRP 安装状态"),
  )
  .post(
    "/frp/download",
    async () => {
      frpManager.startDownload();
      return { success: true, message: "Download started" };
    },
    routeDoc("下载 FRP"),
  )
  .post(
    "/frp/cancel",
    () => {
      frpManager.cancelDownload();
      return { success: true, message: "Download cancelled" };
    },
    routeDoc("取消下载 FRP"),
  )
  .delete(
    "/frp",
    async () => {
      await frpManager.delete();
      return { success: true, message: "Deleted" };
    },
    routeDoc("删除 FRP"),
  )
  .get(
    "/dnsmasq/status",
    async () => {
      const status = await dnsmasqManager.getStatus();
      return { success: true, data: status };
    },
    routeDoc("获取 dnsmasq 状态"),
  )
  .post(
    "/dnsmasq/install",
    async ({ set }) => {
      if (!getRuntimeCapabilities().smart_connect_available) {
        set.status = 403;
        return {
          success: false,
          message: getCapabilityUnavailableMessage("smart_connect_available"),
        };
      }

      const state = await dnsmasqManager.startInstall();
      return { success: true, data: state };
    },
    routeDoc("安装 dnsmasq"),
  );
