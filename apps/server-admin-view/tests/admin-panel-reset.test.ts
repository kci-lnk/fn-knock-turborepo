/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import { jaJPAdmin } from "../../../packages/i18n/src/messages/admin/ja-JP";
import { koKRAdmin } from "../../../packages/i18n/src/messages/admin/ko-KR";
import { zhCNAdmin } from "../../../packages/i18n/src/messages/admin/zh-CN";
import { zhHantAdmin } from "../../../packages/i18n/src/messages/admin/zh-Hant";
import { enServer } from "../../../packages/i18n/src/messages/server/en";
import { jaJPServer } from "../../../packages/i18n/src/messages/server/ja-JP";
import { koKRServer } from "../../../packages/i18n/src/messages/server/ko-KR";
import { zhCNServer } from "../../../packages/i18n/src/messages/server/zh-CN";
import { zhHantServer } from "../../../packages/i18n/src/messages/server/zh-Hant";
import {
  isProtectedAdminPanelDeploymentTarget,
  protectedAdminPanelDeploymentTargets,
} from "../src/lib/admin-panel-runtime";
import { resolveAdminPanelResetGuide } from "../src/lib/docker-admin-panel-reset";
import type { DeploymentTarget } from "../src/types";

const guideCommands = (target: DeploymentTarget) =>
  resolveAdminPanelResetGuide(target)?.steps.map((step) => step.command) ?? [];

describe("admin panel reset guides", () => {
  it("uses one protected-platform matrix for panel settings and access scopes", () => {
    assert.deepEqual(protectedAdminPanelDeploymentTargets, [
      "docker",
      "openwrt",
      "linux",
      "macos",
      "windows",
    ]);

    for (const target of [
      "fpk",
      "fpk-lite",
      "docker",
      "openwrt",
      "linux",
      "macos",
      "synology",
      "windows",
      "dev",
    ] as const) {
      const protectedTarget = [
        "docker",
        "openwrt",
        "linux",
        "macos",
        "windows",
      ].includes(target);
      assert.equal(
        isProtectedAdminPanelDeploymentTarget(target),
        protectedTarget,
      );
      assert.equal(
        resolveAdminPanelResetGuide(target) !== null,
        protectedTarget,
      );
    }
  });

  it("maps every protected deployment to its native reset command", () => {
    assert.deepEqual(guideCommands("docker"), [
      "ssh root@<docker-host>",
      "cd /opt/fn-knock-docker && docker compose exec -T fn-knock fn-knock-reset-panel-password",
      "docker exec -it \"$(docker ps --filter label=com.docker.compose.service=fn-knock --format '{{.Names}}' | head -n 1)\" fn-knock-reset-panel-password",
    ]);
    assert.deepEqual(guideCommands("openwrt"), [
      "ssh root@<openwrt-host>",
      "fn-knock-reset-panel-password",
    ]);
    assert.deepEqual(guideCommands("linux"), [
      "sudo knock reset-panel-password",
    ]);
    assert.deepEqual(guideCommands("macos"), [
      "sudo knock reset-panel-password",
    ]);
    assert.deepEqual(guideCommands("windows"), [
      '& "$env:ProgramFiles\\Knock 敲门\\fn-knock-service.exe" reset-panel-password',
    ]);
  });

  it("never leaks Docker instructions into non-Docker guides", () => {
    for (const target of ["openwrt", "linux", "macos", "windows"] as const) {
      assert.doesNotMatch(guideCommands(target).join("\n"), /docker|compose/iu);
    }
  });

  it("fails closed instead of falling back to Docker", () => {
    for (const target of ["fpk", "fpk-lite", "synology", "dev"] as const) {
      assert.equal(resolveAdminPanelResetGuide(target), null);
    }
    assert.equal(resolveAdminPanelResetGuide(), null);
  });

  it("provides native reset labels and platform-neutral server errors in every locale", () => {
    const catalogs = [
      [zhCNAdmin, zhCNServer],
      [zhHantAdmin, zhHantServer],
      [enAdmin, enServer],
      [koKRAdmin, koKRServer],
      [jaJPAdmin, jaJPServer],
    ] as const;

    for (const [admin, server] of catalogs) {
      assert.ok(admin.components.dockerAdminGate.resetDescriptionDevice);
      assert.ok(admin.components.dockerAdminGate.resetStepLinux);
      assert.doesNotMatch(
        [
          admin.components.dockerAdminGate.resetDescriptionDevice,
          admin.components.dockerAdminGate.resetDescriptionWindows,
          admin.components.dockerAdminGate.resetStepOpenWrtSsh,
          admin.components.dockerAdminGate.resetStepOpenWrtCommand,
          admin.components.dockerAdminGate.resetStepLinux,
          admin.components.dockerAdminGate.resetStepWindows,
        ].join("\n"),
        /docker|compose|container/iu,
      );
      assert.doesNotMatch(server.dockerAdminDenied, /docker/iu);
      assert.doesNotMatch(server.dockerAdminDeniedDescription, /docker/iu);
      assert.doesNotMatch(server.dockerAdminLoginRequired, /docker/iu);
      assert.doesNotMatch(
        server.admin.dockerPanel.passwordNotNeeded,
        /docker/iu,
      );
      assert.doesNotMatch(
        server.admin.dockerPanel.passwordChangeUnsupported,
        /docker/iu,
      );
    }
  });
});
