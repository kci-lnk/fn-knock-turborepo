import { zhCNCommon } from "./common/zh-CN";
import { zhHantCommon } from "./common/zh-Hant";
import { enCommon } from "./common/en";
import { zhCNLocale } from "./locale/zh-CN";
import { zhHantLocale } from "./locale/zh-Hant";
import { enLocale } from "./locale/en";
import { zhCNShared } from "./shared/zh-CN";
import { zhHantShared } from "./shared/zh-Hant";
import { enShared } from "./shared/en";
import { zhCNAdmin } from "./admin/zh-CN";
import { zhHantAdmin } from "./admin/zh-Hant";
import { enAdmin } from "./admin/en";
import { zhCNAuth } from "./auth/zh-CN";
import { zhHantAuth } from "./auth/zh-Hant";
import { enAuth } from "./auth/en";
import { zhCNServer } from "./server/zh-CN";
import { zhHantServer } from "./server/zh-Hant";
import { enServer } from "./server/en";
import { zhCNGateway } from "./gateway/zh-CN";
import { zhHantGateway } from "./gateway/zh-Hant";
import { enGateway } from "./gateway/en";

export const zhCN = {
  common: zhCNCommon,
  locale: zhCNLocale,
  shared: zhCNShared,
  admin: zhCNAdmin,
  auth: zhCNAuth,
  server: zhCNServer,
  gateway: zhCNGateway,
};

export const zhHant = {
  common: zhHantCommon,
  locale: zhHantLocale,
  shared: zhHantShared,
  admin: zhHantAdmin,
  auth: zhHantAuth,
  server: zhHantServer,
  gateway: zhHantGateway,
};

export const en = {
  common: enCommon,
  locale: enLocale,
  shared: enShared,
  admin: enAdmin,
  auth: enAuth,
  server: enServer,
  gateway: enGateway,
};

export const messages = {
  "zh-CN": zhCN,
  "zh-Hant": zhHant,
  en,
} as const;

export type I18nMessageSchema = typeof zhCN;
