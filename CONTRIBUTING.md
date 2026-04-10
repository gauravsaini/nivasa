# Contributing to Nivasa

Thanks for helping improve Nivasa. This repo is still under active build-out, so the main goal is to keep changes small, honest, and aligned with the SCXML contract.

## Working Rules

1. Keep changes focused. Small, reviewable slices are preferred over broad refactors.
1. Do not bypass SCXML-gated flows. If a lifecycle or request-path change is needed, update the statechart first, then code against it.
1. Keep docs truthful. If a feature is only partially landed, say so plainly.
1. Prefer repo-local patterns over introducing new abstractions unless the current surface clearly needs them.

## Verification

Before opening a PR or handing off a change, run the relevant checks for the files you touched.

Typical commands:

```bash
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo test -p nivasa-statechart --test generated_statecharts
cargo run -p nivasa-cli -- statechart validate --all
cargo run -p nivasa-cli -- statechart parity
```

If your change only touches one crate, prefer the narrowest command that still proves the behavior.

## SCXML Expectations

Statechart files under `statecharts/` are the source of truth for lifecycle behavior.

1. Update the `.scxml` file first when a lifecycle change is needed.
1. Rebuild or re-run the relevant SCXML checks.
1. Keep runtime code and tests aligned with the generated statechart, not the other way around.

## Docs Expectations

1. Keep documentation current with shipped behavior, not planned behavior.
1. Call out runtime boundaries explicitly when a feature is only partially wired.
1. Prefer linking to the exact file that proves a behavior.

## Example Expectations

The example apps under `examples/` should be minimal and runnable.

1. Include a short `README.md` with run instructions.
1. Keep the example focused on one concept.
1. Make sure generated files or local build artifacts do not get committed.

## Commit Style

1. Use short, descriptive commit messages.
1. Land related files together.
1. Commit often when a slice is green.

## Before You Push

1. Re-run the smallest meaningful verification set.
1. Check `git status` and make sure only the intended files are staged.
1. Do not overwrite unrelated work from other contributors or agents.
