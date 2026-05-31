# Releasing Nivasa

This document describes how to cut a new release of the Nivasa framework.

## Prerequisites

- All CI checks pass on the `master` branch (check, test, clippy, fmt, docs,
  coverage ≥95%, benchmarks, SCXML validation/parity, cargo-deny).
- `CHANGELOG.md` is updated with the new version section.
- The `CARGO_REGISTRY_TOKEN` secret is configured in the `crates-io-publish`
  GitHub environment.

## Version Bump

1. Update `version` in the workspace `Cargo.toml`:

   ```toml
   [workspace.package]
   version = "0.2.0"  # or whatever the new version is
   ```

2. Run `cargo check --workspace` to ensure all crates pick up the new version.

3. Update `CHANGELOG.md` — move items from `[Unreleased]` into a new version
   section with today's date.

4. Commit: `git commit -am "release: prepare v0.2.0"`

## Tagging

```bash
git tag v0.2.0
git push origin master --tags
```

## Publishing via GitHub Actions (Recommended)

The release workflow is triggered manually via `workflow_dispatch`:

1. Go to **Actions → Release → Run workflow**.
2. Select `dry_run: "true"` first to verify packaging.
3. If the dry-run succeeds, re-run with `dry_run: "false"` to publish.

The workflow will:
- Run the full release gate (check, test, clippy, fmt, docs, coverage,
  benchmarks, SCXML validation/parity, package mirror checks).
- Publish all 17 crates in dependency order with a configurable inter-crate
  wait (default: 60s) to allow crates.io index propagation.

## Publishing Manually

If GitHub Actions is unavailable:

```bash
# 1. Verify crate ordering
scripts/release-crate-order.sh --packages

# 2. Dry-run (validates packaging without uploading)
scripts/publish-crates.sh --dry-run

# 3. Publish for real (requires CARGO_REGISTRY_TOKEN env var)
export CARGO_REGISTRY_TOKEN="cio_..."
scripts/publish-crates.sh --execute --wait 60
```

## Crate Publishing Order

Crates must be published in dependency order (leaves first):

| #  | Crate               |
|----|---------------------|
| 1  | `nivasa-common`     |
| 2  | `nivasa-filters`    |
| 3  | `nivasa-guards`     |
| 4  | `nivasa-interceptors` |
| 5  | `nivasa-macros`     |
| 6  | `nivasa-routing`    |
| 7  | `nivasa-statechart` |
| 8  | `nivasa-core`       |
| 9  | `nivasa-config`     |
| 10 | `nivasa-graphql`    |
| 11 | `nivasa-scheduling` |
| 12 | `nivasa-validation` |
| 13 | `nivasa-pipes`      |
| 14 | `nivasa-http`       |
| 15 | `nivasa-websocket`  |
| 16 | `nivasa`            |
| 17 | `nivasa-cli`        |

## Post-Release

1. Create a GitHub Release from the tag with the changelog section content.
2. Verify crates appear on [crates.io](https://crates.io/search?q=nivasa).
3. Announce on relevant channels (r/rust, social media, etc.).
4. Start a new `[Unreleased]` section in `CHANGELOG.md`.

## Patch Releases

For bug fixes on a released version:

1. Create a branch from the release tag: `git checkout -b release/v0.1.1 v0.1.0`
2. Cherry-pick or apply the fix.
3. Bump the patch version in workspace `Cargo.toml`.
4. Update `CHANGELOG.md`.
5. Tag and publish as above.

## Yanking a Release

If a published version has a critical defect:

```bash
cargo yank --version 0.1.0 nivasa
cargo yank --version 0.1.0 nivasa-core
# ... repeat for affected crates
```

Then publish a patch release with the fix.

## Troubleshooting

- **"crate version already exists"** — The version was already published.
  Bump the version and try again.
- **"dependency not found on crates.io"** — An upstream crate hasn't
  propagated to the index yet. Increase the `--wait` duration.
- **"package verification failed"** — Use `--no-verify` for first-release
  scenarios where internal path dependencies haven't been published yet.
  The release gate CI checks serve as the verification layer.
