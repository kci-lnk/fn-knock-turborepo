/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  acmeCertificateArchiveFilename,
  acmeCertificateArchiveStem,
} from "../src/lib/acme-download";

describe("ACME certificate archive filenames", () => {
  it("keeps ordinary domain names", () => {
    assert.equal(
      acmeCertificateArchiveFilename("Example.COM"),
      "Example.COM.zip",
    );
  });

  it("uses portable names for wildcard certificates", () => {
    assert.equal(
      acmeCertificateArchiveFilename("*.example.com"),
      "wildcard.example.com.zip",
    );
  });

  it("removes unsafe Windows filename characters", () => {
    assert.equal(acmeCertificateArchiveStem(' bad:*?name. '), "bad___name");
    assert.equal(acmeCertificateArchiveStem("..."), "certificate");
  });
});
