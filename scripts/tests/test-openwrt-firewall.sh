#!/bin/bash
# Run against real OpenWrt ubus/rpcd/UCI/jshn/BusyBox, isolated from the host.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
node --test "${ROOT_DIR}/scripts/tests/openwrt-firewall-luci.test.mjs"
docker run --rm --platform linux/amd64 --entrypoint /bin/sh \
  -v "${ROOT_DIR}:/workspace:ro" \
  "${FN_KNOCK_TEST_OPENWRT_IMAGE:-openwrt/rootfs:x86-64-23.05.5}" \
  /workspace/scripts/tests/fixtures/openwrt-firewall/run.sh
