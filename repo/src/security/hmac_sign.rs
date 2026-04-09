//! HMAC-SHA256 request signing for privileged API endpoints.
//!
//! # Protocol
//!
//! Callers of signed endpoints must include two additional headers:
//!
//! ```text
//! X-Timestamp: <Unix epoch seconds, e.g. 1712345678>
//! X-Signature: <lowercase hex of HMAC-SHA256(secret, "{ts}:{METHOD}:{path}")>
//! ```
//!
//! The timestamp window is ±300 seconds (5 minutes) to allow for
//! minor clock skew while preventing replay attacks.
//!
//! # Example (shell)
//!
//! ```bash
//! TS=$(date +%s)
//! MSG="${TS}:POST:/analytics/export"
//! SIG=$(echo -n "$MSG" | openssl dgst -sha256 -hmac "$HMAC_SECRET" | awk '{print $2}')
//! curl -X POST /analytics/export \
//!      -H "X-Timestamp: $TS" \
//!      -H "X-Signature: $SIG" \
//!      -H "Authorization: Bearer $TOKEN" \
//!      -H "Content-Type: application/json" \
//!      -d '{"format":"csv"}'
//! ```

use actix_web::HttpRequest;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::warn;

use crate::errors::AppError;

type HmacSha256 = Hmac<Sha256>;

/// Maximum clock skew allowed between client and server.
const TIMESTAMP_TOLERANCE_SECS: i64 = 300;

/// Verify the HMAC-SHA256 signature of an incoming request.
///
/// Returns `Ok(())` if the signature is valid and the timestamp is
/// recent.  Returns `AppError::BadRequest` for missing/malformed
/// headers and `AppError::Forbidden` for an invalid signature.
///
/// All failures are logged as `HMAC_VERIFICATION_FAILED` security events.
pub fn verify(req: &HttpRequest, secret: &str) -> Result<(), AppError> {
    let timestamp_str = req
        .headers()
        .get("X-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!(path = %req.path(), "HMAC check: missing X-Timestamp header");
            AppError::BadRequest("X-Timestamp header is required for this endpoint".into())
        })?;

    let signature = req
        .headers()
        .get("X-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!(path = %req.path(), "HMAC check: missing X-Signature header");
            AppError::BadRequest("X-Signature header is required for this endpoint".into())
        })?;

    // ── Timestamp freshness ───────────────────────────────────
    let ts: i64 = timestamp_str.parse().map_err(|_| {
        warn!(path = %req.path(), "HMAC check: non-numeric X-Timestamp");
        AppError::BadRequest("X-Timestamp must be a Unix epoch integer".into())
    })?;

    let server_ts = Utc::now().timestamp();
    let skew = (server_ts - ts).abs();

    if skew > TIMESTAMP_TOLERANCE_SECS {
        warn!(
            path = %req.path(),
            skew_secs = skew,
            "HMAC_VERIFICATION_FAILED: timestamp out of window"
        );
        return Err(AppError::BadRequest(format!(
            "X-Timestamp is {} seconds away from server time (max {})",
            skew, TIMESTAMP_TOLERANCE_SECS
        )));
    }

    // ── HMAC computation ──────────────────────────────────────
    let method = req.method().as_str();
    let path = req.path();
    let message = format!("{}:{}:{}", ts, method, path);

    let expected = compute_hmac_hex(secret, &message);

    // ── Constant-time comparison ──────────────────────────────
    if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
        warn!(
            path    = %path,
            method  = %method,
            "HMAC_VERIFICATION_FAILED: signature mismatch"
        );
        return Err(AppError::Forbidden);
    }

    Ok(())
}

/// Compute `hex(HMAC-SHA256(secret, message))`.
fn compute_hmac_hex(secret: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time byte-slice equality to prevent timing side-channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_roundtrip() {
        let secret = "test-secret-key";
        let msg    = "1712345678:POST:/analytics/export";
        let sig    = compute_hmac_hex(secret, msg);
        assert_eq!(sig.len(), 64); // 32 bytes → 64 hex chars
        assert!(constant_time_eq(sig.as_bytes(), sig.as_bytes()));
    }

    #[test]
    fn constant_time_different_lengths_false() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
