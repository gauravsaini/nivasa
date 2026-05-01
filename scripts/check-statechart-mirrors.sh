#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

root_dir="statecharts"
mirrors=(
  "nivasa-statechart/statecharts"
  "nivasa-core/statecharts"
  "nivasa-cli/statecharts"
)

if [ ! -d "$root_dir" ]; then
  printf '%s\n' "missing root SCXML directory: $root_dir" >&2
  exit 1
fi

for mirror in "${mirrors[@]}"; do
  if [ ! -d "$mirror" ]; then
    printf '%s\n' "missing packaged SCXML mirror: $mirror" >&2
    exit 1
  fi

  if ! diff -qr "$root_dir" "$mirror"; then
    printf '\n%s\n' "SCXML mirror drift detected: $mirror must match $root_dir" >&2
    exit 1
  fi
done

printf '%s\n' "SCXML package mirrors match root statecharts"
