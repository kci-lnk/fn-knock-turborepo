# Dependency and release security checks

Run from the repository root with the Go gateway checkout at the commit selected
for the release (by default, `version.json.gatewayCommit`):

```sh
npm run security:test
FN_KNOCK_SECURITY_REPORT_DIR=dist/security-audit npm run security:audit
npm run security:audit:go
node scripts/security-audit-go.mjs --binary dist/fn-knock-go-release/go-reauth-proxy-linux-amd64
```

`FN_KNOCK_GO_REAUTH_PROXY_DIR` selects the gateway checkout. Release workflows
can select the frozen commit with `FN_KNOCK_GATEWAY_COMMIT` or
`FN_KNOCK_GO_SOURCE_COMMIT`. A dirty or mismatched checkout is rejected. Binary
checks compare embedded commit, toolchain and module versions/checksums against
the source, scan that source for the binary's target platform, and save the
artifact SHA-256 with both scanner reports. They never execute foreign binaries.

The CI security job runs for pull requests, manual runs and the existing daily
schedule. It fetches the current RustSec database and records its Git revision.
The quality and release jobs use a reviewed frozen database for reproducibility;
the scheduled current-database check catches advisories published afterward.
All three Rust lockfiles and the root npm lockfile are covered. Scanner failures
fail the gate; they are not treated as empty vulnerability lists.

## Accepted findings

- **CVE-2023-49092 / RUSTSEC-2023-0071:** only `rsa 0.9.10` and
  `rsa 0.10.0-rc.18` in the backend lockfile are accepted. WebAuthn uses the former
  for public-key verification. SSH authentication uses the latter for private-key
  signing and retains a timing side-channel risk. RSA login remains compatible;
  prefer Ed25519 and trusted SSH hosts. There is no patched upstream version as
  of the 2026-10-22 review. This is a risk acceptance, not a vulnerability fix.
- **RUSTSEC-2024-0436:** `paste 1.0.15` is an unmaintained compile-time dependency.
  Its exact warning is tracked separately from vulnerabilities.

Raw `cargo audit` intentionally still reports RSA. The repository script is the
only place accepting exact findings; unexpected, changed or removed findings
require a new review.

`govulncheck` can report wildcard **GO-2026-5932** OpenPGP symbols in stripped
gateway binaries although OpenPGP is not imported. The binary gate accepts this
classification only for `golang.org/x/crypto v0.57.0`, after validating binary
identity and proving with `go list -deps` that no OpenPGP package is present in
the same source/target. The source scan must also pass. Raw findings and the
absence evidence are retained; an actual import, version change or different
advisory fails the binary gate. This does not broaden the Rust allowlist.

## Release verification

Go source and binary reports are stored under `dist/security-audit/` by default.
CI uploads JSON reports even on failure. Common, macOS, NetBSD and Windows
gateway packaging paths run the binary gate. Container release jobs scan the
final architecture image's OS packages with pinned Trivy and block on HIGH or
CRITICAL findings, including findings without an available fix. A failed scan
blocks manifest/final release promotion; architecture staging tags may already
exist. No automatic deployment or credential rotation is performed.

The 2026-10-22 dependency baseline includes Vite 8.1.6, follow-redirects 1.16.1,
rustls 0.23.46, rustls-webpki 0.103.16, webpki root bundles 1.0.10,
quinn-proto 0.11.18 and zip 9.0.0. The selected gateway uses Go 1.26.10,
Coraza 3.8.1, x/net 0.60.0 and x/crypto 0.57.0. ZIP extraction remains manual;
regressions cover traversal, archive symlinks and privileged Unix mode bits.
TLS regressions verify HTTPS with an explicitly trusted test certificate and
reject untrusted HTTPS/SMTP certificates before SMTP authentication. Existing
SSH integration tests retain RSA, Ed25519 and encrypted-key login coverage.

Passing dependency scans does not establish that running installations have
been updated. Deployment acceptance must check the deployed version, gateway
commit and image digest. Locally replaced/vendored libraries still require
upstream advisory review because module version scanning cannot fully assess
local source changes. Native Windows and NetBSD runtime tests run on their
respective CI runners.
