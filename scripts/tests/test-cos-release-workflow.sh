#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKFLOW="${ROOT_DIR}/.github/workflows/release.yml"

fail() {
  printf '[test-cos-release-workflow] ERROR: %s\n' "$*" >&2
  exit 1
}

line_of() {
  local pattern="$1"
  local line
  line="$(grep -nF -- "${pattern}" "${WORKFLOW}" | head -n1 | cut -d: -f1)"
  [ -n "${line}" ] || fail "workflow is missing: ${pattern}"
  printf '%s\n' "${line}"
}

stage_line="$(line_of '- name: Stage draft GitHub Release')"
docker_line="$(line_of '- name: Publish and verify Docker latest')"
plan_line="$(line_of '- name: Validate Tencent COS publish plan')"
cos_line="$(line_of '- name: Publish to Tencent COS and refresh latest CDN cache')"
public_line="$(line_of '- name: Publish immutable GitHub Release')"

[ "${plan_line}" -lt "${stage_line}" ] || fail "COS plan must be validated before staging the draft"
[ "${stage_line}" -lt "${docker_line}" ] || fail "draft Release must be staged before Docker latest"
[ "${docker_line}" -lt "${cos_line}" ] || fail "Docker latest must complete before COS publication"
[ "${cos_line}" -lt "${public_line}" ] || fail "GitHub Release must be published after COS and CDN"

[ "$(grep -Fc 'gh release edit "${TAG}" --draft=false --latest' "${WORKFLOW}")" = "1" ] ||
  fail "workflow must have exactly one public GitHub Release commit point"
grep -Fq "needs.preflight.outputs.prerelease != 'true'" "${WORKFLOW}" ||
  fail "beta releases must not mutate stable latest channels"
grep -Fq 'release_args+=(--prerelease)' "${WORKFLOW}" ||
  fail "beta GitHub Releases must be marked as prereleases"
grep -Fq 'gh release edit "${TAG}" --draft=false --prerelease' "${WORKFLOW}" ||
  fail "beta GitHub Releases must remain prereleases when published"
grep -Fq "timeout-minutes: 60" "${WORKFLOW}" || fail "publish timeout was not increased"
grep -Fq "group: fn-knock-stable-release" "${WORKFLOW}" ||
  fail "stable release mutations must use one global concurrency group"
grep -Fq "node ./scripts/fn-knock-cos-publish.mjs plan" "${WORKFLOW}" ||
  fail "workflow does not produce a COS dry-run plan"
grep -Fq "node ./scripts/fn-knock-cos-publish.mjs publish" "${WORKFLOW}" ||
  fail "workflow does not publish the COS plan"
grep -Fq "FN_KNOCK_LATEST_URL: https://cor.fnknock.cn/latest.json" "${WORKFLOW}" ||
  fail "workflow does not pin the public latest URL"
grep -Fq 'COS_BUCKET: ${{ vars.COS_BUCKET || secrets.COS_BUCKET }}' "${WORKFLOW}" ||
  fail "COS bucket does not support Variables with a Secrets fallback"
grep -Fq 'COS_ACC: ${{ vars.COS_ACC || secrets.COS_ACC }}' "${WORKFLOW}" ||
  fail "COS acceleration endpoint does not support Variables with a Secrets fallback"
grep -Fq 'COS_SECRETID: ${{ secrets.COS_SECRETID }}' "${WORKFLOW}" ||
  fail "COS SecretId is not sourced from GitHub Secrets"

job_needs() {
  local job="$1"
  awk -v job="${job}" '
    $0 == "  " job ":" { in_job = 1; next }
    in_job && /^  [a-zA-Z0-9_-]+:$/ { exit }
    in_job && /^    needs:/ {
      sub(/^    needs:[[:space:]]*/, "")
      if ($0 == "") {
        getline
        sub(/^[[:space:]]*/, "")
      }
      print
      exit
    }
  ' "${WORKFLOW}"
}

for build_job in build-common build-rust-gnu build-rust-musl windows-unsigned; do
  [ "$(job_needs "${build_job}")" = "preflight" ] ||
    fail "${build_job} must start after preflight without waiting for quality"
done
[ "$(job_needs quality)" = "preflight" ] ||
  fail "quality must start after preflight"
[ "$(job_needs macos)" = "[preflight, build-common]" ] ||
  fail "macOS packages must use the frozen source and shared runtime"
[ "$(job_needs publish)" = "[preflight, quality, assemble, windows-unsigned, macos, docker-manifest]" ] ||
  fail "publish must wait for quality and every release artifact"
grep -Fq "needs.quality.result == 'success'" "${WORKFLOW}" ||
  fail "publish must retain quality as a release gate"
grep -Fq -- '-SkipChecks' "${WORKFLOW}" ||
  fail "Windows release packaging must not repeat checks owned by quality/Windows CI"
grep -Fq 'shared-key: windows-x86_64' "${WORKFLOW}" ||
  fail "Windows release packaging must restore its Rust dependency cache"
grep -Fq "needs.macos.result == 'success'" "${WORKFLOW}" ||
  fail "macOS packages must be a release gate"
grep -Fq 'FN_KNOCK_MACOS_INSTALL_SCRIPT: ${{ github.workspace }}/deploy/macos/install.sh' "${WORKFLOW}" ||
  fail "COS publication must include the macOS installer"

cos_block="$(sed -n "${cos_line},${public_line}p" "${WORKFLOW}")"
printf '%s\n' "${cos_block}" | grep -Fq "if: github.event_name == 'push'" ||
  fail "COS publication must only run for tag push releases"

printf '[test-cos-release-workflow] COS transaction ordering passed\n'
