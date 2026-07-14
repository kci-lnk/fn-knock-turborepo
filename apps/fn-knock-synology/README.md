# fn-knock Synology SPK

This directory contains the native Synology DSM 7 package adapter. It is
separate from the fnOS FPK packages under `apps/fn-knock*` because DSM uses a
different package format, lifecycle environment, privilege model, and desktop
integration.

Build the x86_64 package from the repository root:

```bash
npm run fn-knock:spk:build
```

The generated package is written to
`dist/synology/fn-knock-synology-x86_64-<version>.spk`.

The package runs as the DSM-created `fn-knock-synology` package user. Host
firewall mutation is intentionally disabled: DSM 7 rejects unsigned third-party
packages that request package-wide root execution. DSM's package resource
worker registers the public reverse-proxy port 7999 with the built-in firewall
UI.

Runtime data and secrets are persisted in
`/var/packages/fn-knock-synology/var`. On Synology the administration service
listens only on loopback port 7998; the management UI is exposed exclusively
through the authenticated DSM desktop CGI proxy. Port 7991 is not opened.
The Go reverse-proxy gateway continues to listen publicly on port 7999 by
default; port 7998 is only the private administration backend.
The DSM desktop entry uses
`/webman/3rdparty/fn-knock-synology/index.cgi/`; DSM's root-level
`/3rdparty/...` route is not the package CGI route on DSM 7.
