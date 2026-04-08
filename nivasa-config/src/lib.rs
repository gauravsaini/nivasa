//! # nivasa-config
//!
//! Nivasa framework — config.
//!
//! This crate currently exposes the bootstrap-facing `ConfigModule` marker
//! type. Runtime config loading, `for_root`/`for_feature`, env parsing, and
//! injectable services land in later slices.

/// Minimal public config module marker for the `nivasa-config` crate.
///
/// This type intentionally stays small until the richer configuration runtime
/// lands. It gives downstream crates a concrete public surface to reference
/// without implying that env loading or config service wiring already exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ConfigModule;

impl ConfigModule {
    /// Create a new `ConfigModule` marker.
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigModule;

    #[test]
    fn config_module_is_constructible() {
        assert_eq!(ConfigModule::new(), ConfigModule);
    }

    #[test]
    fn config_module_supports_the_expected_core_traits() {
        let module = ConfigModule::default();

        assert_eq!(module, ConfigModule::new());
        assert_eq!(format!("{module:?}"), "ConfigModule");
    }
}
