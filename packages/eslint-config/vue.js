import js from "@eslint/js";
import eslintConfigPrettier from "eslint-config-prettier";
import vuePlugin from "eslint-plugin-vue";
import globals from "globals";
import tseslint from "typescript-eslint";

/**
 * ESLint's flat configuration for the Vue applications in this repository.
 *
 * TypeScript is parsed inside both standalone modules and Vue SFC script
 * blocks. The recommended correctness rules stay enabled while formatting is
 * delegated to Prettier.
 *
 * @type {import("eslint").Linter.Config[]}
 */
export const vueConfig = [
  {
    ignores: ["dist/**", "node_modules/**"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...vuePlugin.configs["flat/essential"],
  {
    files: ["**/*.{js,mjs,cjs,ts,vue}"],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
          ignoreRestSiblings: true,
          varsIgnorePattern: "^_",
        },
      ],
      "vue/multi-word-component-names": "off",
      // Editors receive a reactive form object and update its nested fields.
      // Replacing the prop itself remains forbidden.
      "vue/no-mutating-props": ["error", { shallowOnly: true }],
    },
  },
  {
    files: ["**/*.{ts,vue}"],
    rules: {
      // TypeScript resolves type-space names (for example RequestInit);
      // ESLint's JavaScript-only no-undef rule cannot distinguish them.
      "no-undef": "off",
    },
  },
  {
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: {
        extraFileExtensions: [".vue"],
        parser: tseslint.parser,
        sourceType: "module",
      },
    },
  },
  eslintConfigPrettier,
];
