import "./browser";
import {
  createScopedFnKnockI18n,
  type CreateFnKnockI18nOptions,
} from "./vue-runtime";

export * from "./vue-runtime";

export const createFnKnockI18n = ({
  scope = "admin",
  defaultLocale,
}: CreateFnKnockI18nOptions = {}) =>
  createScopedFnKnockI18n(scope, { defaultLocale });
