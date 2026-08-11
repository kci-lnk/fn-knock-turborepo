import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  $ref?: string;
  const?: boolean | number | string;
  default?: number;
  enum?: Array<null | number | string>;
  format?: string;
  maximum?: number;
  minimum?: number;
  oneOf?: Schema[];
  pattern?: string;
  properties?: Record<string, Schema>;
  required?: string[];
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{ name?: string; schema?: Schema }>;
  requestBody?: {
    content?: Record<string, { schema?: Schema }>;
  };
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("terminal API contract", () => {
  it("keeps every terminal operation typed", () => {
    for (const [method, path, source] of [
      ["get", "/api/admin/terminal/status", "utoipa"],
      ["post", "/api/admin/terminal/tmux/install", "utoipa"],
      ["get", "/api/admin/terminal/sessions", "utoipa"],
      ["post", "/api/admin/terminal/sessions", "utoipa"],
      ["get", "/api/admin/terminal/sessions/{id}", "utoipa"],
      ["patch", "/api/admin/terminal/sessions/{id}", "utoipa"],
      ["delete", "/api/admin/terminal/sessions/{id}", "utoipa"],
      ["post", "/api/admin/terminal/sessions/{id}/attachments", "utoipa"],
      ["get", "/api/admin/terminal/attachments/{id}/poll", "utoipa"],
      ["post", "/api/admin/terminal/attachments/{id}/input", "utoipa"],
      ["post", "/api/admin/terminal/attachments/{id}/resize", "utoipa"],
      ["delete", "/api/admin/terminal/attachments/{id}", "utoipa"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        source ?? "utoipa-domain",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves tmux state and runtime capabilities", () => {
    const tmux = contract.components.schemas.TerminalTmuxInstallStateData;
    assert.deepEqual(tmux.properties?.status?.enum, [
      "uninstalled",
      "installing",
      "installed",
      "error",
    ]);
    assert.equal(tmux.properties?.progress?.minimum, 0);
    assert.equal(tmux.properties?.progress?.maximum, 100);
    assert.deepEqual(tmux.properties?.detectionSource?.enum, [
      "env-path",
      "absolute-path",
      null,
    ]);
    assert.ok(tmux.required?.includes("detectionSource"));

    const runtime = contract.components.schemas.TerminalRuntimeStatusData;
    assert.equal(runtime.properties?.httpPollingAvailable?.const, true);
    assert.ok(runtime.required?.includes("tmuxDetectionSource"));
  });

  it("preserves session shape, dimensions, and request separation", () => {
    const session = contract.components.schemas.TerminalSessionData;
    assert.deepEqual(session.properties?.status?.enum, [
      "created",
      "attached",
      "detached",
      "stopped",
      "error",
    ]);
    assert.equal(session.properties?.cols?.minimum, 20);
    assert.equal(session.properties?.cols?.maximum, 400);
    assert.equal(session.properties?.rows?.minimum, 8);
    assert.equal(session.properties?.rows?.maximum, 200);
    assert.equal(session.properties?.resume_backend?.const, "tmux");
    assert.ok(session.required?.includes("last_frame_revision"));

    assert.equal(
      contract.paths["/api/admin/terminal/sessions"].post.requestBody
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/TerminalCreateSessionBodyData",
    );
    assert.equal(
      contract.paths["/api/admin/terminal/sessions/{id}"].patch.requestBody
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/TerminalRenameSessionBodyData",
    );
  });

  it("documents long-poll compatibility and nullable output", () => {
    const poll =
      contract.paths["/api/admin/terminal/attachments/{id}/poll"].get;
    const cursor = poll.parameters?.find(
      (parameter) => parameter.name === "cursor",
    )?.schema;
    const timeout = poll.parameters?.find(
      (parameter) => parameter.name === "timeout_ms",
    )?.schema;
    assert.equal(cursor?.default, 0);
    assert.equal(cursor?.oneOf?.[0]?.minimum, 0);
    assert.equal(cursor?.oneOf?.[1]?.pattern, "^\\s*[+-]?\\d+");
    assert.equal(timeout?.default, 15_000);

    const pollResult = contract.components.schemas.TerminalPollResultData;
    assert.ok(pollResult.required?.includes("chunk"));
    assert.equal(
      contract.components.schemas.TerminalOutputChunkData.properties?.cursor
        ?.minimum,
      0,
    );
    assert.equal(
      contract.components.schemas.TerminalAttachmentData.properties?.transport
        ?.const,
      "http-polling",
    );
  });

  it("derives frontend terminal models, requests, and queries", () => {
    const types = readSource("../src/types.ts");
    const api = readSource("../src/lib/api/terminal.ts");
    for (const schema of [
      "TerminalTmuxInstallStateData",
      "TerminalSessionData",
      "TerminalAttachmentData",
      "TerminalOutputChunkData",
      "TerminalRuntimeStatusData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /TerminalSchemas\["TerminalCreateSessionBodyData"\]/u);
    assert.match(api, /get_api_admin_terminal_attachments__id__poll/u);
    assert.match(api, /satisfies TerminalRenameSessionBody/u);
    assert.match(api, /satisfies TerminalInputBody/u);
    assert.match(api, /satisfies TerminalResizeBody/u);
    assert.doesNotMatch(types, /interface TerminalSessionRecord/u);
  });
});
