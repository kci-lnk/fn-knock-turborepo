import { Elysia, t } from "elysia";
import { ipLocationService } from "../lib/ip-location";
import { withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const IP_LOCATION_BATCH_LIMIT = 20;
const getIpLocationRouteTranslator = async (request: Request) =>
  createRequestTranslator(request, await configManager.getLocaleConfig());

export const ipLocationRoutes = new Elysia({
  prefix: "/api/admin/ip-location",
  tags: ["IP Location"],
}).post(
  "/batch",
  async ({ request, body, set }) => {
    const { t } = await getIpLocationRouteTranslator(request);
    if (body.ips.length > IP_LOCATION_BATCH_LIMIT) {
      set.status = 400;
      return {
        success: false,
        message: t("server.ipLocationRoutes.batchLimit", {
          max: IP_LOCATION_BATCH_LIMIT,
        }),
      };
    }

    const items = await ipLocationService.ensureEnqueuedBatch(body.ips);
    return {
      success: true,
      data: {
        items,
      },
    };
  },
  withRouteDoc("批量查询 IP 地理位置", {
    body: t.Object({
      ips: t.Array(t.String()),
    }),
  }),
);
