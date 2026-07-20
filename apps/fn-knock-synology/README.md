# fn-knock Synology SPK

This directory contains the native Synology DSM 7 package adapter. It is
separate from the fnOS FPK packages under `apps/fn-knock*` because DSM uses a
different package format, lifecycle environment, privilege model, and desktop
integration.

Build the x86_64, ARMv8, and ARMv7 packages from the repository root:

```bash
npm run fn-knock:spk:build
```

To build one architecture only, use one of:

```bash
npm run fn-knock:spk:build:x86_64
npm run fn-knock:spk:build:armv8
npm run fn-knock:spk:build:armv7
```

The generated packages are written to
`dist/synology/fn-knock-synology-{x86_64|armv8|armv7}-<version>-<build>.spk`.
Use `npm run fn-knock:spk:build:prepared` to package all three architectures
from an existing prepared runtime without rebuilding shared inputs.

## ARM DSM 7 manual smoke checklist

Run this checklist on both an `armv8` and an `armv7` DSM 7 NAS when hardware is
available:

- Confirm the model's Synology Package Arch, manually install the matching SPK,
  and verify that DSM rejects the other ARM package.
- Start and stop the package from Package Center and confirm that both native
  services stay running under the package account.
- Open **Knock** from the DSM desktop and complete the authenticated CGI launch.
- Confirm that the admin backend listens only on `127.0.0.1:7998` and remains
  reachable through the DSM desktop proxy.
- Configure a gateway mapping and verify service traffic through port `7999`.
- Upgrade from the previous SPK build and confirm that configuration, keys, and
  runtime data are preserved.

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
