import { zhCNCommon } from "./common/zh-CN";
import { zhHantCommon } from "./common/zh-Hant";
import { enCommon } from "./common/en";
import { koKRCommon } from "./common/ko-KR";
import { jaJPCommon } from "./common/ja-JP";
import { zhCNLocale } from "./locale/zh-CN";
import { zhHantLocale } from "./locale/zh-Hant";
import { enLocale } from "./locale/en";
import { koKRLocale } from "./locale/ko-KR";
import { jaJPLocale } from "./locale/ja-JP";
import { zhCNShared } from "./shared/zh-CN";
import { zhHantShared } from "./shared/zh-Hant";
import { enShared } from "./shared/en";
import { koKRShared } from "./shared/ko-KR";
import { jaJPShared } from "./shared/ja-JP";
import { zhCNAdmin } from "./admin/zh-CN";
import { zhHantAdmin } from "./admin/zh-Hant";
import { enAdmin } from "./admin/en";
import { koKRAdmin } from "./admin/ko-KR";
import { jaJPAdmin } from "./admin/ja-JP";
import { zhCNAuth } from "./auth/zh-CN";
import { zhHantAuth } from "./auth/zh-Hant";
import { enAuth } from "./auth/en";
import { koKRAuth } from "./auth/ko-KR";
import { jaJPAuth } from "./auth/ja-JP";
import { zhCNServer } from "./server/zh-CN";
import { zhHantServer } from "./server/zh-Hant";
import { enServer } from "./server/en";
import { koKRServer } from "./server/ko-KR";
import { jaJPServer } from "./server/ja-JP";
import { zhCNGateway } from "./gateway/zh-CN";
import { zhHantGateway } from "./gateway/zh-Hant";
import { enGateway } from "./gateway/en";
import { koKRGateway } from "./gateway/ko-KR";
import { jaJPGateway } from "./gateway/ja-JP";

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

export const koKR = {
  common: koKRCommon,
  locale: koKRLocale,
  shared: koKRShared,
  admin: koKRAdmin,
  auth: koKRAuth,
  server: koKRServer,
  gateway: koKRGateway,
};

export const jaJP = {
  common: jaJPCommon,
  locale: jaJPLocale,
  shared: jaJPShared,
  admin: jaJPAdmin,
  auth: jaJPAuth,
  server: jaJPServer,
  gateway: jaJPGateway,
};

export const messages = {
  "zh-CN": zhCN,
  "zh-Hant": zhHant,
  en,
  "ko-KR": koKR,
  "ja-JP": jaJP,
} as const;

export type I18nMessageSchema = typeof zhCN;
