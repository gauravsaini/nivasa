#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/release-crate-order.sh [--packages|--manifests|--json]

Print publishable workspace crates in dependency order.

Options:
  --packages   Print package names, one per line. Default.
  --manifests  Print manifest paths, one per line.
  --json       Print JSON array with name, version, and manifest_path.
  -h, --help   Show this help.
USAGE
}

format="packages"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --packages)
      format="packages"
      ;;
    --manifests)
      format="manifests"
      ;;
    --json)
      format="json"
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

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

if command -v uv >/dev/null 2>&1; then
  python_cmd=(uv run --quiet python)
elif command -v python3 >/dev/null 2>&1; then
  python_cmd=(python3)
else
  printf '%s\n' "python3 or uv is required to read cargo metadata" >&2
  exit 1
fi

metadata_file="$(mktemp "${TMPDIR:-/tmp}/nivasa-metadata.XXXXXX")"
trap 'rm -f "$metadata_file"' EXIT

cargo metadata --no-deps --format-version 1 >"$metadata_file"

"${python_cmd[@]}" - "$metadata_file" "$format" <<'PY'
import json
import os
import sys
from collections import defaultdict

metadata_path, output_format = sys.argv[1:3]
with open(metadata_path, "r", encoding="utf-8") as handle:
    metadata = json.load(handle)

workspace_root = os.path.realpath(metadata["workspace_root"])
workspace_members = set(metadata["workspace_members"])

packages_by_id = {
    package["id"]: package
    for package in metadata["packages"]
    if package["id"] in workspace_members
}


def is_publishable(package):
    if package.get("publish") == []:
        return False

    manifest = os.path.realpath(package["manifest_path"])
    try:
        relative = os.path.relpath(manifest, workspace_root)
    except ValueError:
        return False

    parts = relative.split(os.sep)
    if parts[0] in {"benches", "examples"}:
        return False

    return True


publishable = {
    package_id: package
    for package_id, package in packages_by_id.items()
    if is_publishable(package)
}
publishable_names = {package["name"] for package in publishable.values()}

required_names = {"nivasa-statechart", "nivasa-graphql", "nivasa-scheduling"}
missing_required = sorted(required_names - publishable_names)
if missing_required:
    joined = ", ".join(missing_required)
    raise SystemExit(f"missing required publishable crates from order: {joined}")

ids_by_name = {package["name"]: package_id for package_id, package in publishable.items()}
dependencies = {package_id: set() for package_id in publishable}
dependents = defaultdict(set)

for package_id, package in publishable.items():
    for dependency in package.get("dependencies", []):
        if dependency.get("kind") == "dev":
            continue

        dependency_path = dependency.get("path")
        dependency_name = dependency.get("name")
        if not dependency_path or dependency_name not in ids_by_name:
            continue

        dependency_id = ids_by_name[dependency_name]
        dependencies[package_id].add(dependency_id)
        dependents[dependency_id].add(package_id)

def order_key(package_id):
    name = publishable[package_id]["name"]
    rank = 0
    if name == "nivasa":
        rank = 1
    elif name == "nivasa-cli":
        rank = 2
    return (rank, name)


ready = sorted(
    [package_id for package_id, deps in dependencies.items() if not deps],
    key=order_key,
)
ordered = []

while ready:
    package_id = ready.pop(0)
    ordered.append(package_id)

    for dependent_id in sorted(dependents[package_id], key=order_key):
        dependencies[dependent_id].discard(package_id)
        if not dependencies[dependent_id] and dependent_id not in ordered and dependent_id not in ready:
            ready.append(dependent_id)
    ready.sort(key=order_key)

if len(ordered) != len(publishable):
    remaining = sorted(
        publishable[package_id]["name"]
        for package_id, deps in dependencies.items()
        if deps
    )
    raise SystemExit(f"dependency cycle in publishable crates: {', '.join(remaining)}")

if output_format == "packages":
    for package_id in ordered:
        print(publishable[package_id]["name"])
elif output_format == "manifests":
    for package_id in ordered:
        print(publishable[package_id]["manifest_path"])
elif output_format == "json":
    print(json.dumps([
        {
            "name": publishable[package_id]["name"],
            "version": publishable[package_id]["version"],
            "manifest_path": publishable[package_id]["manifest_path"],
        }
        for package_id in ordered
    ], indent=2))
else:
    raise SystemExit(f"unsupported output format: {output_format}")
PY
