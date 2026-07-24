import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  DOCKER_ADMIN_PASSWORD_MAX_BYTES,
  DOCKER_ADMIN_PASSWORD_MIN_BYTES,
  getDockerAdminPasswordByteLength,
  validateDockerAdminPassword,
} from "../src/lib/docker-admin-password";

describe("Docker admin password validation", () => {
  it("accepts passwords at the UTF-8 byte boundaries", () => {
    const minimum = "a1你!";
    const maximum = `a1${"你".repeat(42)}`;

    assert.equal(
      getDockerAdminPasswordByteLength(minimum),
      DOCKER_ADMIN_PASSWORD_MIN_BYTES,
    );
    assert.equal(validateDockerAdminPassword(minimum), null);
    assert.equal(
      getDockerAdminPasswordByteLength(maximum),
      DOCKER_ADMIN_PASSWORD_MAX_BYTES,
    );
    assert.equal(validateDockerAdminPassword(maximum), null);
  });

  it("rejects passwords outside the UTF-8 byte boundaries", () => {
    assert.equal(validateDockerAdminPassword("a1你"), "tooShort");
    assert.equal(
      validateDockerAdminPassword(`a1${"你".repeat(43)}`),
      "tooLong",
    );
  });

  it("rejects ASCII and Unicode whitespace", () => {
    assert.equal(validateDockerAdminPassword("abc 123"), "containsWhitespace");
    assert.equal(validateDockerAdminPassword("abc\t123"), "containsWhitespace");
    assert.equal(
      validateDockerAdminPassword("abc\u3000123"),
      "containsWhitespace",
    );
    assert.equal(
      validateDockerAdminPassword("abc\u0085123"),
      "containsWhitespace",
    );
  });

  it("does not reject the BOM code point that Rust accepts", () => {
    assert.equal(validateDockerAdminPassword("abc\uFEFF123"), null);
  });

  it("requires at least one ASCII letter and one number", () => {
    assert.equal(
      validateDockerAdminPassword("abcdef"),
      "missingLetterOrNumber",
    );
    assert.equal(
      validateDockerAdminPassword("123456"),
      "missingLetterOrNumber",
    );
    assert.equal(
      validateDockerAdminPassword("密码1234"),
      "missingLetterOrNumber",
    );
  });

  it("allows symbols and additional non-whitespace Unicode characters", () => {
    assert.equal(validateDockerAdminPassword("a1@#$%"), null);
    assert.equal(validateDockerAdminPassword("a1密码!"), null);
  });
});
