# Publishing Order

Release crates in dependency order so downstream manifests can resolve already
published versions. This list is derived from workspace dependencies in
`cargo metadata`; crates with `publish = []` are intentionally excluded.

## Recommended Order

1. `nivasa-common`
2. `nivasa-macros`
3. `nivasa-statechart`
4. `nivasa-core`
5. `nivasa-filters`
6. `nivasa-guards`
7. `nivasa-interceptors`
8. `nivasa-routing`
9. `nivasa-validation`
10. `nivasa-websocket`
11. `nivasa-config`
12. `nivasa-graphql`
13. `nivasa-pipes`
14. `nivasa-scheduling`
15. `nivasa-http`
16. `nivasa`
17. `nivasa-cli`

## Release Gate

Do not publish directly from a developer machine. Use the release workflow so
the same gates run for every crate:

- workspace check, test, clippy, rustfmt, and docs
- SCXML validation and generated-code parity checks
- coverage threshold check
- benchmark smoke check
- `cargo publish --dry-run` for every publishable crate in the order above

The first release run should be dry-run only. Fix package metadata, package
contents, and dependency/version issues before enabling real publish.

## Manual Publish

Publishing should be manual and approved:

- trigger the publish workflow with `workflow_dispatch`
- require GitHub environment approval before crates.io publish starts
- use `CARGO_REGISTRY_TOKEN` from repository secrets
- publish crates in the order above
- wait for each crate version to appear in the crates.io index before publishing
  downstream crates

## Release Notes

- Publish dependency crates before dependents.
- Publish the CLI last so generated examples and commands can depend on the
  released umbrella crate version.
- `nivasa-benchmarks` is a workspace benchmark harness and is not published.
- Update `CHANGELOG.md` before cutting the tag and GitHub release.
