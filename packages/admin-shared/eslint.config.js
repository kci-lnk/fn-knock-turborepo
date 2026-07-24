import { vueConfig } from "@repo/eslint-config/vue";

export default [
  ...vueConfig,
  {
    ignores: ["src/assets/**"],
  },
];
