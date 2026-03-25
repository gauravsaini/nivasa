//! # nivasa-common
//!
//! Shared types for the Nivasa framework.
//!
//! This crate provides the foundational types used across all Nivasa crates:
//! - `HttpException` and all standard HTTP exception types (400, 401, 403, 404, 500, etc.)
//! - Common result types and error handling utilities
//! - DTO marker traits

pub mod exceptions;
pub mod http_status;

pub use exceptions::HttpException;
pub use http_status::HttpStatus;
