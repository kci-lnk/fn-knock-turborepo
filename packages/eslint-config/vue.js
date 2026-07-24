import js from "@eslint/js";
import eslintConfigPrettier from "eslint-config-prettier";
import vuePlugin from "eslint-plugin-vue";
import vueAccessibilityPlugin from "eslint-plugin-vuejs-accessibility";
import globals from "globals";
import tseslint from "typescript-eslint";
import { vueA11yProjectPlugin } from "./vue-a11y-project.js";

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
  ...vueAccessibilityPlugin.configs["flat/recommended"],
  {
    files: ["**/*.{js,mjs,cjs,ts,vue}"],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    plugins: {
      "project-a11y": vueA11yProjectPlugin,
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
      "no-restricted-globals": [
        "error",
        {
          name: "confirm",
          message:
            "Use the project confirmation dialog. Browser-mandated leave prompts should use beforeunload.",
        },
      ],
      "no-restricted-properties": [
        "error",
        {
          object: "window",
          property: "confirm",
          message:
            "Use the project confirmation dialog. Browser-mandated leave prompts should use beforeunload.",
        },
        {
          object: "globalThis",
          property: "confirm",
          message:
            "Use the project confirmation dialog. Browser-mandated leave prompts should use beforeunload.",
        },
        {
          object: "self",
          property: "confirm",
          message:
            "Use the project confirmation dialog. Browser-mandated leave prompts should use beforeunload.",
        },
      ],
      "vuejs-accessibility/no-aria-hidden-on-focusable": "error",
      "vuejs-accessibility/no-role-presentation-on-focusable": "error",
      // The upstream rule lowercases Vue component names and mistakes our
      // non-rendering <Select> root for a native <select>. The project rule
      // checks the actual focusable primitives, including <SelectTrigger>.
      "vuejs-accessibility/form-control-has-label": "off",
      "project-a11y/form-control-has-accessible-name": "error",
      "project-a11y/interactive-has-accessible-name": "error",
      "vuejs-accessibility/label-has-for": [
        "error",
        {
          // WCAG permits either an explicit `for`/`id` association or a
          // control nested inside the label.
          required: { some: ["nesting", "id"] },
          controlComponents: [
            "Checkbox",
            "ComboboxInput",
            "Input",
            "InputGroupInput",
            "InputGroupTextarea",
            "InputOTP",
            "RadioGroupItem",
            "SelectTrigger",
            "Slider",
            "Switch",
            "TagsInputInput",
            "Textarea",
          ],
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
