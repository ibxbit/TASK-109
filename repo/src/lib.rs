//! VitalPath library crate.
//!
//! Exposes every module publicly so that `#[cfg(test)] mod tests` blocks
//! inside each file can be exercised by `cargo test`, and so that the
//! blackbox integration tests under `tests/` can construct the same
//! types the binary uses. The `src/main.rs` binary is a thin wrapper
//! that re-uses these modules via `use vitalpath::...`.

pub mod api;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod errors;
pub mod metrics;
pub mod middleware;
pub mod models;
pub mod notifications;
pub mod schema;
pub mod security;
