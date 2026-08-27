import { defineConfig, type Plugin } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import { readFileSync } from "node:fs";

const isFpkLiteBuild = process.env.FN_KNOCK_FRONTEND_TARGET === "fpk-lite";
const { version: appVersion } = JSON.parse(
  readFileSync(path.resolve(__dirname, "../../version.json"), "utf8"),
) as { version: string };
const versionedAssetsDir = `assets/v${appVersion}`;

const createChunkMatcher = (patterns: string[]) => (id: string) =>
  patterns.some((pattern) => id.includes(pattern));

const isFrameworkChunk = createChunkMatcher([
  "node_modules/vue/",
  "node_modules/@vue/",
  "node_modules/vue-router/",
  "node_modules/pinia/",
  "node_modules/@vueuse/",
]);

const isInteractionChunk = createChunkMatcher([
  "node_modules/reka-ui/dist/Collection/",
  "node_modules/reka-ui/dist/Primitive/",
  "node_modules/reka-ui/dist/RovingFocus/",
  "node_modules/reka-ui/dist/Tabs/",
  "node_modules/reka-ui/dist/shared/",
]);

const isDashboardCoreChunk = createChunkMatcher([
  "packages/ui-vue/src/components/ui/alert/",
  "packages/ui-vue/src/components/ui/card/",
  "packages/ui-vue/src/components/ui/skeleton/",
  "packages/ui-vue/src/components/ui/tabs/",
  "packages/admin-shared/src/composables/createVisibilityPoller.ts",
  "packages/admin-shared/src/composables/useAsyncAction.ts",
  "packages/admin-shared/src/composables/useDateTimeDisplayState.ts",
  "packages/admin-shared/src/composables/useDelayedLoading.ts",
  "packages/admin-shared/src/utils/formatDateTimeSafe.ts",
  "apps/server-admin-view/src/components/LiveStatusBadge.vue",
  "apps/server-admin-view/src/composables/useTargetPolling.ts",
  "apps/server-admin-view/src/lib/api/dashboard.ts",
  "apps/server-admin-view/src/lib/api/polling.ts",
  "apps/server-admin-view/src/lib/pollingLifecycle.ts",
]);

const createGhosttyExternalWasmPlugin = (): Plugin => ({
  name: "fn-knock:ghostty-external-wasm",
  enforce: "pre",
  transform(code, id) {
    const normalizedId = id.split(path.sep).join("/");
    if (
      !normalizedId.endsWith("/node_modules/ghostty-web/dist/ghostty-web.js")
    ) {
      return null;
    }

    const inlineLoadPattern =
      / {2}static async load\(A\) \{\n {4}if \(A\)\n {6}return q\.loadFromPath\(A\);\n {4}const B = new URL\("data:application\/wasm;base64,[\s\S]*?\n {2}static async loadFromPath\(A\) \{/;
    const externalLoad = `  static async load(A) {
    if (!A)
      throw new Error("ghostty-web requires an explicit WASM URL in this build");
    return q.loadFromPath(A);
  }
  static async loadFromPath(A) {`;
    const nextCode = code.replace(inlineLoadPattern, externalLoad);

    if (nextCode === code) {
      throw new Error("Failed to strip ghostty-web inline WASM fallback");
    }

    return {
      code: nextCode,
      map: null,
    };
  },
});

const isCriticalHtmlPreload = (dependency: string) => {
  const name = path.basename(dependency);
  return (
    name.startsWith("_plugin-vue_export-helper-") ||
    name.startsWith("rolldown-runtime-") ||
    name.startsWith("preload-helper-") ||
    name.startsWith("framework-") ||
    name.startsWith("dashboard-core-") ||
    name.startsWith("interaction-vendor-") ||
    name.startsWith("config-") ||
    name.startsWith("dockerAdminAuth-")
  );
};

export default defineConfig({
  base: "./",
  publicDir: path.resolve(__dirname, "../../packages/icons"),
  plugins: [
    createGhosttyExternalWasmPlugin(),
    vue(),
    tailwindcss({
      optimize: process.env.NODE_ENV !== "development",
    }),
  ],
  optimizeDeps: {
    exclude: ["qrcode.vue"],
  },
  build: {
    manifest: true,
    target: "chrome109",
    cssMinify: "esbuild",
    // fnOS WebViews can retain an immutable module response across an FPK
    // replacement. Namespacing every generated asset by the package version
    // guarantees that an upgrade cannot reuse a representation from an older
    // installation, even when an individual dependency chunk is unchanged.
    assetsDir: versionedAssetsDir,
    modulePreload: {
      resolveDependencies(_filename, dependencies, context) {
        if (context.hostType !== "html") return dependencies;
        return dependencies.filter(isCriticalHtmlPreload);
      },
    },
    rolldownOptions: {
      output: {
        manualChunks(id) {
          if (isFrameworkChunk(id)) return "framework";
          // A module request crosses the fnOS CGI boundary and starts a local
          // curl process. Keep interaction primitives together instead of
          // emitting many sub-kilobyte chunks for the first dashboard render.
          if (isInteractionChunk(id)) return "interaction-vendor";
          if (isDashboardCoreChunk(id)) return "dashboard-core";
        },
      },
    },
  },
  resolve: {
    alias: {
      "@runtime-debug": path.resolve(
        __dirname,
        isFpkLiteBuild
          ? "./src/lib/runtime-overrides-disabled.ts"
          : "./src/lib/docker-debug.ts",
      ),
      "@/components/ui": path.resolve(
        __dirname,
        "../../packages/ui-vue/src/components/ui",
      ),
      "@/lib/utils": path.resolve(
        __dirname,
        "../../packages/ui-vue/src/lib/utils.ts",
      ),
      "@frontend-core": path.resolve(
        __dirname,
        "../../packages/frontend-core/src",
      ),
      "@admin-shared": path.resolve(
        __dirname,
        "../../packages/admin-shared/src",
      ),
      "@fn-knock/i18n/core": path.resolve(
        __dirname,
        "../../packages/i18n/src/core.ts",
      ),
      "@fn-knock/i18n/vue/admin": path.resolve(
        __dirname,
        "../../packages/i18n/src/vue-admin.ts",
      ),
      "@fn-knock/i18n/vue": path.resolve(
        __dirname,
        "../../packages/i18n/src/vue.ts",
      ),
      "@fn-knock/i18n": path.resolve(
        __dirname,
        "../../packages/i18n/src/index.ts",
      ),
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/__fn-knock": {
        target: "http://localhost:7998",
        changeOrigin: true,
      },
      "/api": {
        target: "http://localhost:7998",
        changeOrigin: true,
      },
    },
  },
});
