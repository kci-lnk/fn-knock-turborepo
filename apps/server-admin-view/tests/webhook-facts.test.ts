import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  WEBHOOK_COMMON_FACTS,
  WEBHOOK_EVENT_FACTS,
  WEBHOOK_FACT_LABELS,
} from "../src/views/event-center/notifications/webhook-facts";
import {
  createWebhookSampleContext,
  validateWebhookBodyConfig,
  DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
} from "../src/views/event-center/notifications/webhook-body";

const source = readFileSync(
  new URL(
    "../../server-admin-rs/src/notifications/routes/details.rs",
    import.meta.url,
  ),
  "utf8",
);
const keysIn = (text: string) =>
  [...text.matchAll(/&mut facts,\s*"([^"]+)"/gu)].map((match) => match[1]!);

describe("webhook detail variable catalogue", () => {
  it("covers every event and common detail without duplicate keys", () => {
    const branches = [...source.matchAll(/^ {8}("FN_EVENT_[\s\S]*?) => \{/gmu)];
    const expected = branches.flatMap((branch, index) => {
      const end =
        branches[index + 1]?.index ??
        source.indexOf("        _ => {}", branch.index);
      const keys = keysIn(source.slice(branch.index! + branch[0].length, end));
      assert.equal(new Set(keys).size, keys.length);
      assert.ok(keys.every((key) => !WEBHOOK_COMMON_FACTS.includes(key)));
      return [...branch[1]!.matchAll(/"(FN_EVENT_[^"]+)"/gu)].map((match) => ({
        event: match[1],
        keys,
      }));
    });
    assert.deepEqual(WEBHOOK_EVENT_FACTS, expected);
    assert.deepEqual(
      WEBHOOK_COMMON_FACTS,
      keysIn(source.slice(source.indexOf("        _ => {}"))),
    );
    assert.deepEqual(
      Object.keys(WEBHOOK_FACT_LABELS).sort(),
      [...new Set(keysIn(source))].sort(),
    );
    for (const [key, label] of Object.entries(WEBHOOK_FACT_LABELS)) {
      assert.equal(
        key,
        label.replace(/[A-Z]/gu, (letter) => `_${letter.toLowerCase()}`),
      );
    }
  });

  it("provides sample details and accepts every independent variable", () => {
    const sample = JSON.parse(createWebhookSampleContext());
    assert.equal(sample.message.fact_values.login_ip, sample.event.payload.ip);
    assert.deepEqual(
      Object.values(sample.message.fact_values),
      sample.message.facts.map((fact: { value: string }) => fact.value),
    );
    for (const key of Object.keys(WEBHOOK_FACT_LABELS)) {
      assert.deepEqual(
        validateWebhookBodyConfig(
          {
            mode: "custom",
            format: "json",
            template: JSON.stringify({
              value: `{{message.fact_values.${key}}}`,
            }),
          },
          DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
          "provider",
        ),
        [],
      );
    }
  });
});

it("keeps all scoped admin detail labels aligned with server translations", async () => {
  for (const locale of ["zh-CN", "zh-Hant", "en", "ja-JP", "ko-KR"]) {
    const { default: scoped } = await import(
      `../../../packages/i18n/src/messages/scopes/admin/${locale}.ts`
    );
    const serverModule = await import(
      `../../../packages/i18n/src/messages/server/${locale}.ts`
    );
    const server = Object.values(serverModule)[0] as {
      notifications: {
        templates: { details: { facts: Record<string, string> } };
      };
    };
    for (const label of Object.values(WEBHOOK_FACT_LABELS)) {
      if (label === "host") continue;
      assert.equal(
        typeof scoped.admin.notifications.body.factLabels[label],
        "string",
      );
      assert.equal(
        scoped.admin.notifications.body.factLabels[label],
        server.notifications.templates.details.facts[label],
      );
    }
  }
});
