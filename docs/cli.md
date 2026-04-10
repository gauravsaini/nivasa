# CLI

`nivasa-cli` is the repo tool for scaffolding apps and working with SCXML.

## Commands

### `nivasa info`

Prints framework version, Rust version, and OS info.

### `nivasa new <project-name>`

Creates a starter app layout with:

1. `Cargo.toml`
1. `src/main.rs`
1. `src/app_module.rs`
1. `.env`
1. `.gitignore`
1. Default SCXML files under `statecharts/`

Example:

```bash
cargo run -p nivasa-cli -- new myapp
```

### `nivasa generate ...`

Generator commands are available through `nivasa generate` and the short alias `nivasa g`.

Supported generators:

1. `module`
1. `controller`
1. `service`
1. `guard`
1. `interceptor`
1. `pipe`
1. `filter`
1. `middleware`
1. `resource`

Examples:

```bash
cargo run -p nivasa-cli -- g module users
cargo run -p nivasa-cli -- g controller users
cargo run -p nivasa-cli -- g resource users
```

The generators currently create file skeletons and templates. The resource generator creates a module, controller, service, and DTO files under a dedicated directory.

### `nivasa statechart ...`

The SCXML subcommands are the other major CLI surface:

1. `validate --all`
1. `validate <file>`
1. `parity`
1. `visualize --format svg`
1. `diff HEAD~1`
1. `inspect --host 127.0.0.1 --port 3000`

Example:

```bash
cargo run -p nivasa-cli -- statechart validate --all
cargo run -p nivasa-cli -- statechart parity
```

## Practical Boundaries

1. `new` and the generators are scaffolding tools, not full project installers.
1. `statechart validate` and `statechart parity` are part of the repo's SCXML gatekeeping story.
1. The CLI currently prints or writes files; it does not yet provide the full app `listen` runtime.

## Suggested Workflow

1. Scaffold a project with `nivasa new myapp`.
1. Add a controller or resource with `nivasa g ...`.
1. Keep SCXML files in sync with `nivasa statechart validate --all`.
1. Use `nivasa statechart parity` before landing statechart changes.
