import {
  registerScopedLocaleLoaders,
  type LocaleLoaderMap,
} from "./browser-runtime";

const authLocaleLoaders: LocaleLoaderMap = {
  "zh-CN": () => import("./messages/scopes/auth/zh-CN"),
  "zh-Hant": () => import("./messages/scopes/auth/zh-Hant"),
  en: () => import("./messages/scopes/auth/en"),
  "ko-KR": () => import("./messages/scopes/auth/ko-KR"),
};

registerScopedLocaleLoaders("auth", authLocaleLoaders);

export { authLocaleLoaders };
