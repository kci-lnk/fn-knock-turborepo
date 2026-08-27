import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_LOCALE, LOCALE_STORAGE_KEY } from "@fn-knock/i18n/core";

const loadRuntime = async (
  loaders: Partial<
    Record<
      "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP",
      () => Promise<{ default: Record<string, unknown> }>
    >
  >,
) => {
  const browserRuntime =
    await import("../../../packages/i18n/src/browser-runtime");
  browserRuntime.registerScopedLocaleLoaders("admin", {
    "zh-CN": loaders["zh-CN"] ?? vi.fn(),
    "zh-Hant": loaders["zh-Hant"] ?? vi.fn(),
    en: loaders.en ?? vi.fn(),
    "ko-KR": loaders["ko-KR"] ?? vi.fn(),
    "ja-JP": loaders["ja-JP"] ?? vi.fn(),
  });
  return import("../../../packages/i18n/src/vue-runtime");
};

beforeEach(() => {
  vi.resetModules();
  window.localStorage.clear();
  document.cookie = "fn_knock_locale=; Max-Age=0; Path=/";
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("i18n application bootstrap", () => {
  it("falls back to the default locale when a persisted locale chunk fails", async () => {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "en");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const runtime = await loadRuntime({
      en: async () => {
        throw new TypeError("Failed to fetch");
      },
      "zh-CN": async () => ({ default: { common: { ok: "确定" } } }),
    });

    const i18n = await runtime.createScopedFnKnockI18n("admin");

    expect(i18n.global.locale.value).toBe(DEFAULT_LOCALE);
    expect(window.localStorage.getItem(LOCALE_STORAGE_KEY)).toBe(
      DEFAULT_LOCALE,
    );
    expect(document.documentElement.lang).toBe(DEFAULT_LOCALE);
    expect(warn).toHaveBeenCalledOnce();
  });

  it("surfaces the error when both the preferred and default chunks fail", async () => {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "en");
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const defaultFailure = new TypeError("Load failed");
    const runtime = await loadRuntime({
      en: async () => {
        throw new TypeError("Failed to fetch");
      },
      "zh-CN": async () => {
        throw defaultFailure;
      },
    });

    await expect(runtime.createScopedFnKnockI18n("admin")).rejects.toBe(
      defaultFailure,
    );
  });
});
