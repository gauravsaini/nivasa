# Publishing Order

Release crates in dependency order so downstream manifests can resolve already
published versions.

## Recommended Order

1. `nivasa-common`
2. `nivasa-core`
3. `nivasa-macros`
4. `nivasa-http`
5. `nivasa-routing`
6. `nivasa-guards`
7. `nivasa-interceptors`
8. `nivasa-pipes`
9. `nivasa-filters`
10. `nivasa-validation`
11. `nivasa-config`
12. `nivasa-websocket`
13. `nivasa`
14. `nivasa-cli`

## Release Notes

- Publish leaf crates first.
- Publish the umbrella crate after all sub-crates are available.
- Publish the CLI last so generated examples and commands can depend on the
  released umbrella crate version.
- Update `CHANGELOG.md` before cutting the tag and GitHub release.
