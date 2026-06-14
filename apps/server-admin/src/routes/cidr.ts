import { Elysia, t } from "elysia";
import { CidrServiceError, cidrService, withCidrLocale } from "../lib/cidr";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

type RequestTranslator = ReturnType<typeof createRequestTranslator>["t"];

const getCidrRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

const handleCidrError = (
  error: unknown,
  t: RequestTranslator,
): { status: number; message: string } => {
  if (error instanceof CidrServiceError) {
    return {
      status: error.statusCode,
      message: error.message,
    };
  }

  return {
    status: 500,
    message:
      error instanceof Error ? error.message : t("server.cidr.serviceError"),
  };
};

export const cidrRoutes = new Elysia({
  prefix: "/api/admin/cidr",
  tags: ["CIDR"],
})
  .get(
    "/provinces",
    async ({ request, set }) => {
      const { locale, t } = await getCidrRouteTranslator(request);
      try {
        const payload = await withCidrLocale(locale, () =>
          cidrService.getProvinces(),
        );
        return { success: true, data: payload };
      } catch (error) {
        const handled = handleCidrError(error, t);
        set.status = handled.status;
        return { success: false, message: handled.message };
      }
    },
    routeDoc("获取省份列表"),
  )
  .get(
    "/cities",
    async ({ query, request, set }) => {
      const { locale, t } = await getCidrRouteTranslator(request);
      try {
        const payload = await withCidrLocale(locale, () =>
          cidrService.getCities(query.province),
        );
        return { success: true, data: payload };
      } catch (error) {
        const handled = handleCidrError(error, t);
        set.status = handled.status;
        return { success: false, message: handled.message };
      }
    },
    withRouteDoc("获取指定省份的城市列表", {
      query: t.Object({
        province: t.String(),
      }),
    }),
  )
  .get(
    "/selector",
    async ({ query, request, set }) => {
      const { locale, t } = await getCidrRouteTranslator(request);
      try {
        const payload = await withCidrLocale(locale, () =>
          cidrService.getSelector(query.province),
        );
        return { success: true, data: payload };
      } catch (error) {
        const handled = handleCidrError(error, t);
        set.status = handled.status;
        return { success: false, message: handled.message };
      }
    },
    withRouteDoc("获取省市联动选择器数据", {
      query: t.Object({
        province: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/cidrs",
    async ({ query, request, set }) => {
      const { locale, t } = await getCidrRouteTranslator(request);
      try {
        const payload = await withCidrLocale(locale, () =>
          cidrService.getCidrs({
            province: query.province,
            city: query.city,
          }),
        );
        return { success: true, data: payload };
      } catch (error) {
        const handled = handleCidrError(error, t);
        set.status = handled.status;
        return { success: false, message: handled.message };
      }
    },
    withRouteDoc("查询省市对应的 CIDR 列表", {
      query: t.Object({
        province: t.String(),
        city: t.Optional(t.String()),
      }),
    }),
  );
