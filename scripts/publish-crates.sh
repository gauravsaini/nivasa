#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/publish-crates.sh [--dry-run|--execute] [OPTIONS]

Publish workspace crates in dependency order. Defaults to dry-run.

Modes:
  --dry-run          List packaged files with `cargo package --list`. Default.
                     This is safe before the first workspace crates exist on crates.io.
  --execute          Run real `cargo publish` for each crate in computed order.
  --list             Print the computed publish order and exit.

Options:
  --wait SECONDS     Sleep between crates. Useful while crates.io index updates.
  --registry NAME    Publish to a named Cargo registry during --execute.
  --index URL        Publish to a registry index URL during --execute.
  --token TOKEN      Pass an explicit registry token to Cargo during --execute.
  --allow-dirty      Pass `--allow-dirty` to Cargo.
  --verify           Ask Cargo to perform registry-aware package/publish verification.
                     Use after workspace dependencies exist on crates.io.
  --skip-preflight   Skip release preflight checks.
  -h, --help         Show this help.

Preflight:
  By default the script runs SCXML guards before dry-run or execute:
    cargo run --locked --package nivasa-cli -- statechart validate --all
    cargo run --locked --package nivasa-cli -- statechart parity

Environment:
  CARGO_PACKAGE_EXTRA_ARGS  Extra args appended to `cargo package` in --dry-run.
  CARGO_PUBLISH_EXTRA_ARGS  Extra args appended to `cargo publish` in --execute.
  NIVASA_RELEASE_SKIP_PREFLIGHT=1 also skips preflight.
USAGE
}

mode="dry-run"
wait_seconds="0"
registry=""
index_url=""
token=""
allow_dirty=false
verify=false
skip_preflight=false

case "${NIVASA_RELEASE_SKIP_PREFLIGHT:-}" in
  1|true|TRUE|yes|YES)
    skip_preflight=true
    ;;
esac

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      mode="dry-run"
      ;;
    --execute)
      mode="execute"
      ;;
    --list)
      mode="list"
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
    --index)
      if [ "$#" -lt 2 ]; then
        printf '%s\n' "--index needs URL" >&2
        exit 2
      fi
      index_url="$2"
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
    --verify)
      verify=true
      ;;
    --skip-preflight)
      skip_preflight=true
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

if [ -n "$registry" ] && [ -n "$index_url" ]; then
  printf '%s\n' "--registry and --index are mutually exclusive" >&2
  exit 2
fi

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

print_order() {
  printf 'crate count: %s\n' "${#crates[@]}"
  printf '%s\n' "crate order:"
  printf '  %s\n' "${crates[@]}"
}

print_command() {
  printf 'command:'
  local redact_next=false
  local arg
  for arg in "$@"; do
    if [ "$redact_next" = true ]; then
      printf ' %q' '<redacted>'
      redact_next=false
      continue
    fi

    printf ' %q' "$arg"
    if [ "$arg" = "--token" ]; then
      redact_next=true
    fi
  done
  printf '\n'
}

run_scxml_preflight() {
  printf '\n%s\n' "preflight: validating SCXML statecharts"
  cargo run --locked --package nivasa-cli -- statechart validate --all

  printf '\n%s\n' "preflight: checking SCXML/generated-code parity"
  cargo run --locked --package nivasa-cli -- statechart parity
}

append_extra_args() {
  local env_name="$1"
  local value="${!env_name:-}"

  if [ -n "$value" ]; then
    # Intentionally preserve the existing shell-style escape hatch for release operators.
    # shellcheck disable=SC2206
    local extra_args=($value)
    command+=("${extra_args[@]}")
  fi
}

if [ "$mode" = "list" ]; then
  print_order
  exit 0
fi

printf 'release mode: %s\n' "$mode"
if [ "$mode" = "dry-run" ] && [ "$verify" = false ]; then
  printf '%s\n' "cargo verification: package file-list mode"
elif [ "$verify" = true ]; then
  printf '%s\n' "cargo verification: enabled"
else
  printf '%s\n' "cargo verification: disabled for publish (--no-verify)"
fi
print_order

if [ "$mode" = "dry-run" ] && { [ -n "$registry" ] || [ -n "$index_url" ] || [ -n "$token" ]; }; then
  printf '%s\n' "dry-run note: registry, index, and token options are only used by --execute."
fi

if [ "$skip_preflight" = false ]; then
  run_scxml_preflight
else
  printf '\n%s\n' "preflight: skipped"
fi

for index in "${!crates[@]}"; do
  crate="${crates[$index]}"

  if [ "$mode" = "dry-run" ]; then
    command=(cargo package --locked --package "$crate")
    if [ "$verify" = true ]; then
      :
    else
      command+=(--list)
    fi
    if [ "$allow_dirty" = true ]; then
      command+=(--allow-dirty)
    fi
    append_extra_args CARGO_PACKAGE_EXTRA_ARGS
  else
    command=(cargo publish --locked --package "$crate")
    if [ "$verify" = false ]; then
      command+=(--no-verify)
    fi
    if [ -n "$registry" ]; then
      command+=(--registry "$registry")
    fi
    if [ -n "$index_url" ]; then
      command+=(--index "$index_url")
    fi
    if [ -n "$token" ]; then
      command+=(--token "$token")
    fi
    if [ "$allow_dirty" = true ]; then
      command+=(--allow-dirty)
    fi
    append_extra_args CARGO_PUBLISH_EXTRA_ARGS
  fi

  printf '\n[%s/%s] %s\n' "$((index + 1))" "${#crates[@]}" "$crate"
  print_command "${command[@]}"
  "${command[@]}"

  if [ "$wait_seconds" -gt 0 ] && [ "$index" -lt "$((${#crates[@]} - 1))" ]; then
    printf 'waiting %ss for registry/index propagation\n' "$wait_seconds"
    sleep "$wait_seconds"
  fi
done
