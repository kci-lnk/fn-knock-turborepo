import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

type ModuleInfoLookup = (id: string) => {
  importers: readonly string[];
} | null;

// Rolldown can otherwise emit a separate shared chunk for every slightly
// different combination of auth routes. Follow static importer chains so the
// small auth-flow modules can be grouped without pulling optional dynamic
// imports (locales, ALTCHA, and the PoW worker) into the initial bundle.
const isStaticallyImportedBy = (
  moduleId: string,
  getModuleInfo: ModuleInfoLookup,
  matchesImporter: (id: string) => boolean,
  visited = new Set<string>(),
): boolean => {
  if (visited.has(moduleId)) return false;
  visited.add(moduleId);

  const moduleInfo = getModuleInfo(moduleId);
  if (!moduleInfo) return false;

  return moduleInfo.importers.some(
    (importer) =>
      matchesImporter(importer) ||
      isStaticallyImportedBy(importer, getModuleInfo, matchesImporter, visited),
  );
};

export default defineConfig({
  base: "./",
  publicDir: path.resolve(__dirname, "../../packages/icons"),
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: function (tag) {
            return tag === "altcha-widget";
          },
        },
      },
    }),
    tailwindcss(),
  ],
  build: {
    manifest: true,
    cssMinify: "esbuild",
    rollupOptions: {
      output: {
        codeSplitting: {
          groups: [
            // Auth is a small, single-entry app. Keeping its static bootstrap
            // together avoids several tiny module-preload requests.
            {
              name: "auth-initial",
              tags: ["$initial"],
              priority: 100,
            },
            // Keep Home's dependencies separate from the login-only flow so
            // visiting the authenticated landing page does not download the
            // complete login form.
            {
              includeDependenciesRecursively: false,
              priority: 10,
              test: (moduleId) =>
                !moduleId
                  .replaceAll("\\", "/")
                  .includes("/node_modules/altcha/"),
              name(moduleId, context) {
                const normalizedId = moduleId.replaceAll("\\", "/");
                if (normalizedId.includes("/src/views/")) return null;
                const getModuleInfo = (id: string) => context.getModuleInfo(id);

                const importedByView = (view: string) =>
                  isStaticallyImportedBy(
                    moduleId,
                    getModuleInfo,
                    (importer) =>
                      importer
                        .replaceAll("\\", "/")
                        .split("?", 1)[0]
                        .endsWith(`/src/views/${view}.vue`),
                  );

                if (importedByView("Home")) return "auth-home";
                if (importedByView("Login")) return "auth-login";

                return null;
              },
            },
          ],
        },
      },
    },
  },
  resolve: {
    alias: {
      "@/components/ui": path.resolve(
        __dirname,
        "../../packages/ui-vue/src/components/ui",
      ),
      "@/lib/utils": path.resolve(
        __dirname,
        "../../packages/ui-vue/src/lib/utils.ts",
      ),
      "@admin-shared": path.resolve(
        __dirname,
        "../../packages/admin-shared/src",
      ),
      "@frontend-core": path.resolve(
        __dirname,
        "../../packages/frontend-core/src",
      ),
      "@fn-knock/i18n/core": path.resolve(
        __dirname,
        "../../packages/i18n/src/core.ts",
      ),
      "@fn-knock/i18n/vue/auth": path.resolve(
        __dirname,
        "../../packages/i18n/src/vue-auth.ts",
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
        target: "http://localhost:7997",
        changeOrigin: true,
      },
      "/api": {
        target: "http://localhost:7997",
        changeOrigin: true,
      },
    },
  },
});
