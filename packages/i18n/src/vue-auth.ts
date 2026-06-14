import "./browser-auth";
import {
  createScopedFnKnockI18n,
  type CreateFnKnockI18nOptions,
} from "./vue-runtime";

export * from "./vue-runtime";

export const createFnKnockI18n = (
  options: Omit<CreateFnKnockI18nOptions, "scope"> & { scope?: "auth" } = {},
) => createScopedFnKnockI18n("auth", options);
