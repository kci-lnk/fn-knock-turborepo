import { Elysia } from "elysia";
import { updateManager } from "../lib/update-manager";
import { routeDoc } from "../lib/openapi";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
} from "../lib/runtime-profile";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const getUpdateRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

export const updateRoutes = new Elysia({
  prefix: "/api/admin/update",
  tags: ["Update"],
})
  .get(
    "/status",
    async () => {
      const data = await updateManager.getStatus();
      return { success: true, data };
    },
    routeDoc("获取更新状态"),
  )
  .post(
    "/check",
    async () => {
      await updateManager.checkNow("manual");
      const data = await updateManager.getStatus();
      return { success: true, data };
    },
    routeDoc("检查更新"),
  )
  .post(
    "/download",
    async ({ set, request }) => {
      const { locale, t } = await getUpdateRouteTranslator(request);

      if (!getRuntimeCapabilities().self_update_available) {
        set.status = 403;
        return {
          success: false,
          message: getCapabilityUnavailableMessage(
            "self_update_available",
            undefined,
            locale,
          ),
        };
      }

      try {
        await updateManager.triggerDownload();
        const data = await updateManager.getStatus();
        return {
          success: true,
          message: t("server.updateRoutes.downloadStarted"),
          data,
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.updateRoutes.downloadStartFailed"),
        };
      }
    },
    routeDoc("下载更新包"),
  )
  .post(
    "/install",
    async ({ set, request }) => {
      const { locale, t } = await getUpdateRouteTranslator(request);

      if (!getRuntimeCapabilities().self_update_available) {
        set.status = 403;
        return {
          success: false,
          message: getCapabilityUnavailableMessage(
            "self_update_available",
            undefined,
            locale,
          ),
        };
      }

      try {
        await updateManager.triggerInstall();
        return {
          success: true,
          message: t("server.updateRoutes.installStarted"),
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.updateRoutes.installStartFailed"),
        };
      }
    },
    routeDoc("安装更新"),
  )
  .post(
    "/check-and-download",
    async ({ set, request }) => {
      const { locale, t } = await getUpdateRouteTranslator(request);

      if (!getRuntimeCapabilities().self_update_available) {
        set.status = 403;
        return {
          success: false,
          message: getCapabilityUnavailableMessage(
            "self_update_available",
            undefined,
            locale,
          ),
        };
      }

      try {
        await updateManager.checkNow("manual-check-and-download");
        await updateManager.triggerDownload();
        const data = await updateManager.getStatus();
        return {
          success: true,
          message: t("server.updateRoutes.checkAndDownloadStarted"),
          data,
        };
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error
              ? error.message
              : t("server.updateRoutes.startFailed"),
        };
      }
    },
    routeDoc("检查并下载更新"),
  )
  .get(
    "/confirm",
    async () => {
      const data = await updateManager.consumeConfirmMessage();
      return { success: true, data };
    },
    routeDoc("获取更新确认信息"),
  );
