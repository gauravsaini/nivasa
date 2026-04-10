# Configuration

`nivasa-config` provides the current config surface for Nivasa. It is still
small on purpose, but the pieces that exist are real and usable today.

## What Exists Now

- `ConfigOptions` for bootstrap-time settings.
- `ConfigModule::load_env(&ConfigOptions)` for loading `.env` files into an in-memory map.
- `ConfigService` for read-only lookup from that map.
- `ConfigModule::for_root(...)` and `ConfigModule::for_feature(...)` for dynamic-module metadata.

## Environment Loading

`ConfigModule::load_env` reads configured env files in order and merges them
into a `BTreeMap<String, String>`.

Current behavior:

- `ignore_env_file = true` returns an empty map.
- No env file paths means no file loading.
- Later files override earlier keys.
- When `expand_variables = true`, values can reference other keys with `$VAR`
  or `${VAR}` before process env overlay.
- Process environment values are applied last and override file values.

Example:

```rust
use nivasa_config::{ConfigModule, ConfigOptions};

let options = ConfigOptions::new()
    .with_env_file_path(".env")
    .with_expand_variables(true);

let values = ConfigModule::load_env(&options)?;
```

## Type-Safe Lookup

`ConfigService` wraps the loaded map and exposes read-only accessors:

- `get_raw(key)` returns `Option<&str>`.
- `get::<T>(key)` parses values with `FromStr`.
- `get_or_default(key, default)` falls back when missing or invalid.
- `get_or_throw(key)` returns an owned string or `ConfigException::MissingKey`.

This is current type safety today. Schema validation is not implemented yet.

## Current Gaps

These are still future work:

- startup validation against a config schema
- required-key validation at module init
- `#[derive(ConfigSchema)]`
- richer failure reporting for invalid config shapes

## Minimal Example

```rust
use nivasa_config::{ConfigModule, ConfigOptions, ConfigService};

let options = ConfigOptions::new().with_env_file_path(".env");
let values = ConfigModule::load_env(&options)?;
let config = ConfigService::from_values(values);

let port: u16 = config.get("APP_PORT").unwrap_or(3000);
let name = config.get_or_throw("APP_NAME")?;
```
