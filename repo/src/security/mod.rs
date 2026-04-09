//! Security subsystem.
//!
//! Modules:
//! - [`masking`]    — identifier masking for structured logs
//! - [`rate_limit`] — sliding-window per-principal rate limiting middleware
//! - [`hmac_sign`]  — HMAC-SHA256 request signing for privileged endpoints

pub mod hmac_sign;
pub mod masking;
pub mod rate_limit;
