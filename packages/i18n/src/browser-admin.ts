import {
  registerScopedLocaleLoaders,
  type LocaleLoaderMap,
} from "./browser-runtime";

const adminLocaleLoaders: LocaleLoaderMap = {
  "zh-CN": () => import("./messages/scopes/admin/zh-CN"),
  "zh-Hant": () => import("./messages/scopes/admin/zh-Hant"),
  en: () => import("./messages/scopes/admin/en"),
  "ko-KR": () => import("./messages/scopes/admin/ko-KR"),
  "ja-JP": () => import("./messages/scopes/admin/ja-JP"),
};

registerScopedLocaleLoaders("admin", adminLocaleLoaders);

export { adminLocaleLoaders };
