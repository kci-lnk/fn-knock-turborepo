/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  acmeCertificateArchiveFilename,
  acmeCertificateOutputPaths,
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
    assert.equal(acmeCertificateArchiveStem(" bad:*?name. "), "bad___name");
    assert.equal(acmeCertificateArchiveStem("..."), "certificate");
  });
});

it("previews output filenames for POSIX roots, Windows drives and wildcard domains", () => {
  assert.deepEqual(acmeCertificateOutputPaths("/", "*.Example.COM"), {
    certificatePath: "/wildcard.example.com.cert.pem",
    privateKeyPath: "/wildcard.example.com.key.pem",
  });
  assert.equal(
    acmeCertificateOutputPaths("C:\\certs\\", "example.com").privateKeyPath,
    "C:\\certs\\example.com.key.pem",
  );
});
