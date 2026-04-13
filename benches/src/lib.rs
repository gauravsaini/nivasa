//! Benchmark harness support for the Nivasa workspace.

/// Return the benchmark crate name so the package has a tiny stable export.
pub fn crate_name() -> &'static str {
    "nivasa-benchmarks"
}
