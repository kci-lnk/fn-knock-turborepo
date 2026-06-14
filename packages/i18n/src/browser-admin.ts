import {
  registerScopedLocaleLoaders,
  type LocaleLoaderMap,
} from "./browser-runtime";

const adminLocaleLoaders: LocaleLoaderMap = {
  "zh-CN": () => import("./messages/scopes/admin/zh-CN"),
  "zh-Hant": () => import("./messages/scopes/admin/zh-Hant"),
  en: () => import("./messages/scopes/admin/en"),
};

registerScopedLocaleLoaders("admin", adminLocaleLoaders);

export { adminLocaleLoaders };
