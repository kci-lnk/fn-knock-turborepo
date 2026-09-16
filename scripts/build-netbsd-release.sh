#!/bin/sh
# Runs inside the pinned NetBSD Actions VM after platform-independent inputs exist.
set -eu
[ "$(uname -s)" = NetBSD ] && [ "$(uname -m)" = amd64 ] || {
  echo 'NetBSD/amd64 is required' >&2
  exit 1
}
ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"
: "${FN_KNOCK_VERSION:?version is required}"
: "${FN_KNOCK_GATEWAY_COMMIT:?gateway commit is required}"
: "${CARGO_TARGET_DIR:?use a target directory outside the synced workspace}"
case "$FN_KNOCK_VERSION" in *[!0-9A-Za-z.-]*|'') exit 1 ;; esac
manifest=apps/server-admin-rs/Cargo.toml
cargo test --locked --manifest-path "$manifest" --lib runtime_health::debug_resources::tests
cargo test --locked --manifest-path "$manifest" --lib current_process_rss_is_reported
cargo test --locked --manifest-path "$manifest" --lib infra::openapi_docs::tests
cargo build --locked --release --manifest-path "$manifest" --bin server-admin-rs

stage=$(mktemp -d /tmp/fn-knock-netbsd-stage.XXXXXX)
trap 'rm -rf "$stage"' EXIT HUP INT TERM
app="$stage/fn-knock"
mkdir -p "$app/bin" "$app/ui/www" "$app/server-auth-view/dist" "$app/server/server-admin/resources"
cp "$CARGO_TARGET_DIR/release/server-admin-rs" "$app/bin/"
cp dist/netbsd-input/go-reauth-proxy "$app/bin/"
chmod 755 "$app/bin/server-admin-rs" "$app/bin/go-reauth-proxy"
cp -R apps/server-admin-view/dist/. "$app/ui/www/"
cp -R apps/server-auth-view/dist/. "$app/server-auth-view/dist/"
cp apps/server-admin-rs/resources/acmesh.zip "$app/server/server-admin/resources/"
cp deploy/netbsd/README.md "$app/README.md"
printf '%s\n' "$FN_KNOCK_VERSION" > "$app/VERSION"
printf '%s\n' "$FN_KNOCK_GATEWAY_COMMIT" > "$app/GATEWAY_COMMIT"
test -s "$app/ui/www/index.html"
test -s "$app/server-auth-view/dist/index.html"
# Package must depend only on the NetBSD base runtime, not CI's pkgsrc libraries.
ldd "$app/bin/server-admin-rs" > "$stage/ldd.txt"
cat "$stage/ldd.txt"
if grep -E 'not found|/usr/pkg/|/opt/rust/' "$stage/ldd.txt"; then
  echo 'Unexpected dynamic dependency in NetBSD release' >&2
  exit 1
fi
mkdir -p dist/netbsd
archive="$ROOT_DIR/dist/netbsd/fn-knock-netbsd-${FN_KNOCK_VERSION}-amd64.tar.gz"
tar -czf "$archive" -C "$stage" fn-knock
sh scripts/smoke-netbsd-release.sh "$archive"
