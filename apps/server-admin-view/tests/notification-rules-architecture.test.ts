import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const rulesTabSource = readSource(
  "../src/views/event-center/notifications/RulesTab.vue",
);
const controllerSource = readSource(
  "../src/views/event-center/notifications/useNotificationRules.ts",
);
const resourceSource = readSource(
  "../src/views/event-center/notifications/useNotificationRulesResource.ts",
);
const editorSource = readSource(
  "../src/views/event-center/notifications/useNotificationRuleEditor.ts",
);

describe("notification rules architecture", () => {
  it("keeps the SFC focused on presentation and existing child components", () => {
    assert.match(rulesTabSource, /useNotificationRules/u);
    assert.match(rulesTabSource, /<RulesListTable/u);
    assert.match(rulesTabSource, /<SchemaFieldsEditor/u);
    assert.doesNotMatch(rulesTabSource, /EventCenterAPI\./u);
    assert.doesNotMatch(rulesTabSource, /buildRulePayload/u);
  });

  it("separates resource mutations from the editor workflow", () => {
    assert.match(controllerSource, /useNotificationRulesResource/u);
    assert.match(controllerSource, /useNotificationRuleEditor/u);
    assert.match(resourceSource, /getNotificationRules/u);
    assert.match(resourceSource, /deleteNotificationRule/u);
    assert.doesNotMatch(resourceSource, /buildRulePayload/u);
    assert.match(editorSource, /createNotificationRule/u);
    assert.match(editorSource, /updateNotificationRule/u);
    assert.match(editorSource, /buildRulePayload/u);
  });
});
