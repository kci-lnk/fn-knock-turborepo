import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{ name?: string }>;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  paths: Record<string, Record<string, Operation>>;
};

describe("Web terminal API contract", () => {
  it("publishes local, SSH target, session, and attachment operations", () => {
    for (const [method, path] of [
      ["get", "/api/admin/terminal/local"],
      ["patch", "/api/admin/terminal/local"],
      ["post", "/api/admin/terminal/local/sessions"],
      ["get", "/api/admin/terminal/targets"],
      ["post", "/api/admin/terminal/targets"],
      ["get", "/api/admin/terminal/targets/{id}"],
      ["patch", "/api/admin/terminal/targets/{id}"],
      ["delete", "/api/admin/terminal/targets/{id}"],
      ["post", "/api/admin/terminal/targets/probe-host-key"],
      ["post", "/api/admin/terminal/targets/test-connection"],
      ["get", "/api/admin/terminal/sessions"],
      ["post", "/api/admin/terminal/targets/{id}/sessions"],
      ["patch", "/api/admin/terminal/sessions/{id}"],
      ["delete", "/api/admin/terminal/sessions/{id}"],
      ["post", "/api/admin/terminal/sessions/{id}/attachments"],
      ["get", "/api/admin/terminal/attachments/{id}/events"],
      ["post", "/api/admin/terminal/attachments/{id}/input"],
      ["post", "/api/admin/terminal/attachments/{id}/resize"],
      ["post", "/api/admin/terminal/attachments/{id}/control"],
      ["delete", "/api/admin/terminal/attachments/{id}"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps force and long-poll queries typed", () => {
    const localParameters =
      contract.paths["/api/admin/terminal/local"]?.patch?.parameters ?? [];
    assert.ok(localParameters.some((parameter) => parameter.name === "force"));
    assert.ok(
      localParameters.some(
        (parameter) => parameter.name === "confirmationToken",
      ),
    );
    for (const method of ["patch", "delete"] as const) {
      const parameters =
        contract.paths["/api/admin/terminal/targets/{id}"]?.[method]
          ?.parameters ?? [];
      assert.ok(parameters.some((parameter) => parameter.name === "force"));
      assert.ok(
        parameters.some((parameter) => parameter.name === "confirmationToken"),
      );
    }
    assert.ok(
      contract.paths[
        "/api/admin/terminal/targets/{id}"
      ]?.delete?.parameters?.some((parameter) => parameter.name === "revision"),
    );
    const events =
      contract.paths["/api/admin/terminal/attachments/{id}/events"]?.get;
    assert.ok(
      events?.parameters?.some((parameter) => parameter.name === "after"),
    );
    assert.ok(
      events?.parameters?.some((parameter) => parameter.name === "timeoutMs"),
    );
  });

  it("keeps the new local PTY API while removing tmux and legacy polling endpoints", () => {
    for (const path of [
      "/api/admin/terminal/status",
      "/api/admin/terminal/tmux/install",
      "/api/admin/terminal/sessions",
      "/api/admin/terminal/attachments/{id}/poll",
    ]) {
      if (path === "/api/admin/terminal/sessions") {
        assert.equal(contract.paths[path]?.post, undefined);
      } else {
        assert.equal(contract.paths[path], undefined, path);
      }
    }

    const api = readSource("../src/lib/api/terminal.ts");
    assert.doesNotMatch(api, /tmux|terminal\/status|\/poll["`]/iu);
    assert.match(api, /\/terminal\/local/u);
    assert.match(api, /\/terminal\/local\/sessions/u);
    assert.match(api, /probe-host-key/u);
    assert.match(api, /test-connection/u);
    assert.match(api, /\/events/u);
  });
});
