#!/bin/bash

fn_knock_app_version() {
  local root_dir="$1"
  local version_file="${root_dir}/version.json"
  local version

  [ -f "${version_file}" ] || {
    echo "missing version file: ${version_file}" >&2
    return 1
  }

  version="$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${version_file}" | head -n1)"
  [ -n "${version}" ] || {
    echo "failed to parse version from ${version_file}" >&2
    return 1
  }

  printf '%s\n' "${version}"
}

fn_knock_sync_manifest_version() {
  local root_dir="$1"
  local manifest_file="$2"
  local log_prefix="$3"
  local app_version
  local current_manifest_version
  local tmp_manifest

  app_version="$(fn_knock_app_version "${root_dir}")" || return 1
  [ -f "${manifest_file}" ] || {
    echo "${log_prefix} Missing manifest file: ${manifest_file}" >&2
    return 1
  }

  current_manifest_version="$(sed -nE 's/^version=(.*)$/\1/p' "${manifest_file}" | head -n1)"
  if [ "${current_manifest_version}" = "${app_version}" ]; then
    echo "${log_prefix} Manifest version is already up to date: ${app_version}"
    return 0
  fi

  tmp_manifest="$(mktemp)"
  awk -v version="${app_version}" '
    BEGIN { updated = 0 }
    /^version=/ {
      print "version=" version
      updated = 1
      next
    }
    { print }
    END {
      if (!updated) {
        print "version=" version
      }
    }
  ' "${manifest_file}" > "${tmp_manifest}"
  mv "${tmp_manifest}" "${manifest_file}"

  echo "${log_prefix} Synced manifest version: ${current_manifest_version:-<empty>} -> ${app_version}"
}

fn_knock_sync_cargo_package_version() {
  local root_dir="$1"
  local cargo_toml="$2"
  local log_prefix="$3"
  local app_version
  local current_cargo_version
  local tmp_cargo_toml

  app_version="$(fn_knock_app_version "${root_dir}")" || return 1
  [ -f "${cargo_toml}" ] || {
    echo "${log_prefix} Missing Cargo manifest: ${cargo_toml}" >&2
    return 1
  }

  current_cargo_version="$(
    awk '
      /^\[package\]/ { in_package = 1; next }
      /^\[/ && in_package { in_package = 0 }
      in_package && /^[[:space:]]*version[[:space:]]*=/ {
        value = $0
        sub(/^[^"]*"/, "", value)
        sub(/".*$/, "", value)
        print value
        exit
      }
    ' "${cargo_toml}"
  )"
  if [ "${current_cargo_version}" = "${app_version}" ]; then
    echo "${log_prefix} Cargo package version is already up to date: ${app_version}"
    return 0
  fi

  tmp_cargo_toml="$(mktemp)"
  awk -v version="${app_version}" '
    BEGIN { in_package = 0; updated = 0 }
    /^\[package\]/ {
      in_package = 1
      print
      next
    }
    /^\[/ && in_package {
      if (!updated) {
        print "version = \"" version "\""
        updated = 1
      }
      in_package = 0
      print
      next
    }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
      print "version = \"" version "\""
      updated = 1
      next
    }
    { print }
    END {
      if (in_package && !updated) {
        print "version = \"" version "\""
      }
    }
  ' "${cargo_toml}" > "${tmp_cargo_toml}"
  mv "${tmp_cargo_toml}" "${cargo_toml}"

  echo "${log_prefix} Synced Cargo package version: ${current_cargo_version:-<empty>} -> ${app_version}"
}

fn_knock_sync_rust_package_version() {
  local root_dir="$1"
  local log_prefix="$2"

  fn_knock_sync_cargo_package_version \
    "${root_dir}" \
    "${root_dir}/apps/server-admin-rs/Cargo.toml" \
    "${log_prefix}"
}
