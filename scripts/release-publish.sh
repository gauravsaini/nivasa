#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/release-publish.sh [--dry-run|--execute] [--wait SECONDS] [--registry NAME] [--token TOKEN] [--allow-dirty]

Publish workspace crates in dependency order. Defaults to dry-run.

Options:
  --dry-run       Run `cargo publish --dry-run`. Default.
  --execute       Run real `cargo publish`.
  --wait SECONDS  Sleep between crates. Useful while crates.io index updates.
  --registry NAME Publish to a named Cargo registry.
  --token TOKEN   Pass an explicit registry token to Cargo.
  --allow-dirty   Pass `--allow-dirty` to Cargo.
  -h, --help      Show this help.

Extra Cargo args can be passed with CARGO_PUBLISH_EXTRA_ARGS.
USAGE
}

mode="dry-run"
wait_seconds="0"
registry=""
token=""
allow_dirty=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      mode="dry-run"
      ;;
    --execute)
      mode="execute"
      ;;
    --wait)
      if [ "$#" -lt 2 ]; then
        printf '%s\n' "--wait needs seconds" >&2
        exit 2
      fi
      wait_seconds="$2"
      shift
      ;;
    --registry)
      if [ "$#" -lt 2 ]; then
        printf '%s\n' "--registry needs name" >&2
        exit 2
      fi
      registry="$2"
      shift
      ;;
    --token)
      if [ "$#" -lt 2 ]; then
        printf '%s\n' "--token needs value" >&2
        exit 2
      fi
      token="$2"
      shift
      ;;
    --allow-dirty)
      allow_dirty=true
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

case "$wait_seconds" in
  ''|*[!0-9]*)
    printf '%s\n' "--wait must be a non-negative integer" >&2
    exit 2
    ;;
esac

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

order_script="$repo_root/scripts/release-crate-order.sh"
if [ ! -x "$order_script" ]; then
  printf '%s\n' "missing executable order helper: $order_script" >&2
  exit 1
fi

crates=()
while IFS= read -r crate_name; do
  crates+=("$crate_name")
done < <("$order_script" --packages)
if [ "${#crates[@]}" -eq 0 ]; then
  printf '%s\n' "no publishable crates found" >&2
  exit 1
fi

printf 'release mode: %s\n' "$mode"
printf 'crate count: %s\n' "${#crates[@]}"
printf '%s\n' "crate order:"
printf '  %s\n' "${crates[@]}"

for index in "${!crates[@]}"; do
  crate="${crates[$index]}"
  command=(cargo publish --locked --package "$crate")

  if [ "$mode" = "dry-run" ]; then
    command+=(--dry-run)
  fi
  if [ -n "$registry" ]; then
    command+=(--registry "$registry")
  fi
  if [ -n "$token" ]; then
    command+=(--token "$token")
  fi
  if [ "$allow_dirty" = true ]; then
    command+=(--allow-dirty)
  fi
  if [ -n "${CARGO_PUBLISH_EXTRA_ARGS:-}" ]; then
    # shellcheck disable=SC2206
    extra_args=($CARGO_PUBLISH_EXTRA_ARGS)
    command+=("${extra_args[@]}")
  fi

  printf '\n[%s/%s] %s\n' "$((index + 1))" "${#crates[@]}" "${command[*]}"
  "${command[@]}"

  if [ "$wait_seconds" -gt 0 ] && [ "$index" -lt "$((${#crates[@]} - 1))" ]; then
    printf 'waiting %ss for registry/index propagation\n' "$wait_seconds"
    sleep "$wait_seconds"
  fi
done
