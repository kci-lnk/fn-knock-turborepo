#!/bin/bash

# This helper is sourced by the fn-knock Lite build and deploy entrypoints.
# The exported marker is inherited by nested entrypoints so a deploy performs
# the mandatory synchronization once before any build or remote operation.
if [ "${FN_KNOCK_LITE_GRPC_SYNC_GO_COMPLETED:-0}" != "1" ]; then
  FN_KNOCK_LITE_SYNC_ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  printf '[fn-knock-lite] Synchronizing Go gRPC contract before operation...\n'
  bash "${FN_KNOCK_LITE_SYNC_ROOT_DIR}/scripts/sync-go-grpc-contract.sh"
  export FN_KNOCK_LITE_GRPC_SYNC_GO_COMPLETED=1
  unset FN_KNOCK_LITE_SYNC_ROOT_DIR
fi
