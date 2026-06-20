import { Elysia, t } from "elysia";
import {
  MaintenanceBackupError,
  maintenanceBackupService,
} from "../../lib/maintenance-backup";
import { routeDoc, withRouteDoc } from "../../lib/openapi";
import { adminT, getAdminRouteTranslator } from "./shared";

export const adminMaintenanceRoutes = new Elysia()
  .get(
    "/maintenance/backup/export",
    async () => {
      const archive = await maintenanceBackupService.exportBackupArchive();
      const body = new Blob([Uint8Array.from(archive.buffer)], {
        type: "application/octet-stream",
      });

      return new Response(body, {
        headers: {
          "Content-Type": "application/octet-stream",
          "Content-Disposition": `attachment; filename="${archive.filename}"`,
          "Cache-Control": "no-store",
        },
      });
    },
    routeDoc("导出系统备份归档"),
  )
  .get(
    "/maintenance/backup/files",
    async ({ request, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      try {
        return {
          success: true,
          data: await maintenanceBackupService.listBackupDirectoryFiles(),
        };
      } catch (error: any) {
        set.status = 500;
        return {
          success: false,
          message:
            error?.message || adminT(t, "backup.readFnosDirectoryFailed"),
        };
      }
    },
    routeDoc("获取飞牛备份目录文件列表"),
  )
  .post(
    "/maintenance/backup/export/fnos",
    async ({ request, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      try {
        const result =
          await maintenanceBackupService.exportBackupArchiveToDirectory();
        return {
          success: true,
          data: result,
          message: adminT(t, "backup.exportFnosSuccess"),
        };
      } catch (error: any) {
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status = status;
        return {
          success: false,
          message: error?.message || adminT(t, "backup.exportFnosFailed"),
        };
      }
    },
    routeDoc("导出备份到飞牛目录"),
  )
  .post(
    "/maintenance/backup/import",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      try {
        const result = await maintenanceBackupService.importBackupArchive(body);
        return {
          success: true,
          data: result,
          message:
            result.warnings.length > 0
              ? adminT(t, "backup.importSuccessWithWarnings")
              : adminT(t, "backup.importSuccess"),
        };
      } catch (error: any) {
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status = status;
        return {
          success: false,
          message: error?.message || adminT(t, "backup.importFailed"),
        };
      }
    },
    withRouteDoc("导入本地备份归档", {
      body: t.Object({
        filename: t.Optional(t.String()),
        archive_base64: t.String(),
      }),
    }),
  )
  .post(
    "/maintenance/backup/import/fnos",
    async ({ request, body, set }) => {
      const { t } = await getAdminRouteTranslator(request);
      try {
        const result =
          await maintenanceBackupService.importBackupArchiveFromDirectory(
            body.path,
          );
        return {
          success: true,
          data: result,
          message:
            result.warnings.length > 0
              ? adminT(t, "backup.importFnosSuccessWithWarnings")
              : adminT(t, "backup.importFnosSuccess"),
        };
      } catch (error: any) {
        const message = error?.message || adminT(t, "backup.importFnosFailed");
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status =
          error?.code === "ENOENT"
            ? 404
            : error?.code === "EACCES"
              ? 403
              : status;
        return {
          success: false,
          message,
        };
      }
    },
    withRouteDoc("从飞牛目录导入备份", {
      body: t.Object({
        path: t.String(),
      }),
    }),
  );
