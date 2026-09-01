import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { Linter } from "eslint";
import vueParser from "vue-eslint-parser";
import { vueA11yProjectPlugin } from "../../../packages/eslint-config/vue-a11y-project.js";

const ruleName = "project-a11y/no-static-form-field-id-in-loop";

const lintTemplate = (source: string) =>
  new Linter().verify(
    source,
    [
      {
        files: ["**/*.vue"],
        languageOptions: {
          parser: vueParser,
          parserOptions: { sourceType: "module" },
        },
        plugins: { "project-a11y": vueA11yProjectPlugin },
        rules: { [ruleName]: "error" },
      },
    ],
    { filename: "fixture.vue" },
  );

describe("form field id lint guard", () => {
  it("rejects fixed label targets and control ids inside v-for", () => {
    const messages = lintTemplate(`
      <template>
        <div v-for="field in fields">
          <Label for="fixed-field">Field</Label>
          <Input id="fixed-field" />
        </div>
      </template>
    `).filter((message) => message.ruleId === ruleName);

    assert.equal(messages.length, 2);
  });

  it("accepts ids derived from the active loop binding", () => {
    const messages = lintTemplate(`
      <template>
        <div v-for="field in fields">
          <Label :for="'field-' + field.key">Field</Label>
          <Input :id="'field-' + field.key" />
        </div>
      </template>
    `).filter((message) => message.ruleId === ruleName);

    assert.deepEqual(messages, []);
  });
});
