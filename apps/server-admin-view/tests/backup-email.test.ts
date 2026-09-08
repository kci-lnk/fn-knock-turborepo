import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  backupEmailPayload,
  defaultBackupEmail,
  isBackupEmailValid,
} from "../src/lib/backup-email";
describe("backup email", () => {
  it("defaults to disabled and preserves stored passwords in outgoing updates", () => {
    const config = defaultBackupEmail();
    assert.equal(config.enabled, false);
    const payload = backupEmailPayload({
      ...config,
      password_configured: true,
      password: "",
    });
    assert.equal("password" in payload, false);
    assert.equal("password_configured" in payload, false);
    assert.equal(
      backupEmailPayload({ ...config, clear_password: true }).clear_password,
      true,
    );
    assert.equal(
      backupEmailPayload({ ...config, password: "replacement" }).password,
      "replacement",
    );
  });
  it("checks limits and requires connection and recipient fields when enabled", () => {
    const config = defaultBackupEmail();
    assert.equal(isBackupEmailValid(config), true);
    assert.equal(isBackupEmailValid({ ...config, enabled: true }), false);
    assert.equal(
      isBackupEmailValid({ ...config, attachment_limit_mib: 101 }),
      false,
    );
    assert.equal(
      isBackupEmailValid({ ...config, smtp: { ...config.smtp, port: 1.5 } }),
      false,
    );
    assert.equal(
      isBackupEmailValid({
        ...config,
        enabled: true,
        from_address: "backup@example.com",
        to_addresses: ["admin@example.com"],
        smtp: { ...config.smtp, host: "localhost", auth_mode: "none" },
      }),
      true,
    );
  });
});

it("toggling password clearing off restores the original update payload", () => {
  const form = defaultBackupEmail();
  assert.deepEqual(
    backupEmailPayload({ ...form, clear_password: false, password: "" }),
    backupEmailPayload(form),
  );
});
