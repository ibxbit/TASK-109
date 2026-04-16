use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use diesel::prelude::*;
use tracing::{info, warn};

use crate::errors::AppError;

/// AES-256-GCM authenticated field cipher.
/// One instance is created at startup and shared across all requests
/// via `web::Data<FieldCipher>`.
pub struct FieldCipher {
    inner: Aes256Gcm,
    /// Version label of the active key (e.g. "v1").
    /// Written alongside encrypted fields so staleness can be detected.
    pub key_version: String,
}

// Aes256Gcm is Send + Sync — safe to share across threads.
unsafe impl Send for FieldCipher {}
unsafe impl Sync for FieldCipher {}

impl FieldCipher {
    /// Construct from exactly 32 key bytes.
    pub fn new(key: &[u8; 32], key_version: impl Into<String>) -> Self {
        let k = Key::<Aes256Gcm>::from_slice(key);
        Self {
            inner:       Aes256Gcm::new(k),
            key_version: key_version.into(),
        }
    }

    /// Encrypt `plaintext`.
    ///
    /// Returns `(ciphertext_b64, nonce_b64)` — both must be stored together
    /// and provided to `decrypt`. Each call uses a freshly generated nonce.
    pub fn encrypt(&self, plaintext: &str) -> Result<(String, String), AppError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bit random nonce
        let ciphertext = self
            .inner
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Encryption failed")))?;

        Ok((B64.encode(&ciphertext), B64.encode(nonce.as_slice())))
    }

    /// Decrypt ciphertext using the paired nonce. Both values must be the
    /// base64 strings returned by `encrypt`.
    pub fn decrypt(&self, ciphertext_b64: &str, nonce_b64: &str) -> Result<String, AppError> {
        let ciphertext = B64
            .decode(ciphertext_b64)
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid ciphertext encoding")))?;

        let nonce_bytes = B64
            .decode(nonce_b64)
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid nonce encoding")))?;

        if nonce_bytes.len() != 12 {
            return Err(AppError::Internal(anyhow::anyhow!("Nonce must be 12 bytes")));
        }

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = self
            .inner
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Decryption failed — key mismatch or tampered data")))?;

        String::from_utf8(plaintext)
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Decrypted value is not valid UTF-8")))
    }
}

// ── Key-rotation health check ─────────────────────────────────

/// Number of days before the key rotation warning fires.
pub const KEY_ROTATION_DAYS: i64 = 180;

/// Check when the encryption key was last rotated.
///
/// Reads the most recent `key_rotation_logs` row.  If it is older than
/// [`KEY_ROTATION_DAYS`] days (or absent), emits a structured warning
/// so operators can schedule a rotation.
///
/// Errors from this check are non-fatal: a warning is logged and the
/// application continues.
pub fn check_key_rotation(conn: &mut PgConnection) {
    use crate::schema::key_rotation_logs;

    let result: Result<Option<chrono::DateTime<Utc>>, _> = key_rotation_logs::table
        .select(key_rotation_logs::rotated_at)
        .order(key_rotation_logs::rotated_at.desc())
        .first::<chrono::DateTime<Utc>>(conn)
        .optional();

    match result {
        Err(e) => {
            warn!(error = %e, "KEY_ROTATION_CHECK_FAILED: could not query key_rotation_logs");
        }
        Ok(None) => {
            warn!(
                "KEY_ROTATION_CHECK_FAILED: no rows found in key_rotation_logs; \
                 insert an initial row or run migration 00011"
            );
        }
        Ok(Some(last_rotated)) => {
            let age_days = (Utc::now() - last_rotated).num_days();
            if age_days >= KEY_ROTATION_DAYS {
                warn!(
                    last_rotated = %last_rotated.format("%Y-%m-%d"),
                    age_days     = age_days,
                    threshold    = KEY_ROTATION_DAYS,
                    "SECURITY_KEY_ROTATION_NEEDED: encryption key is overdue for rotation"
                );
            } else {
                let days_until = KEY_ROTATION_DAYS - age_days;
                info!(
                    last_rotated  = %last_rotated.format("%Y-%m-%d"),
                    days_until_rotation = days_until,
                    "Key rotation status: OK"
                );
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — FieldCipher roundtrip, tamper detection, error paths.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic 32-byte test key — zero bytes + 1..31 to distinguish
    /// from an all-zero key that some libraries treat as weak.
    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, byte) in k.iter_mut().enumerate() {
            *byte = i as u8;
        }
        k
    }

    #[test]
    fn encrypt_decrypt_roundtrip_ascii() {
        let c = FieldCipher::new(&test_key(), "v1");
        let plaintext = "hello, world";
        let (ct, nonce) = c.encrypt(plaintext).expect("encrypt");
        let decrypted = c.decrypt(&ct, &nonce).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_decrypt_roundtrip_unicode() {
        let c = FieldCipher::new(&test_key(), "v1");
        let plaintext = "日本語 — emoji 🦀 — and a null\0byte";
        let (ct, nonce) = c.encrypt(plaintext).expect("encrypt");
        let decrypted = c.decrypt(&ct, &nonce).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn each_encryption_produces_fresh_nonce() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (ct1, n1) = c.encrypt("same").unwrap();
        let (ct2, n2) = c.encrypt("same").unwrap();
        // Ciphertexts and nonces differ even though plaintext is identical.
        assert_ne!(ct1, ct2);
        assert_ne!(n1, n2);
    }

    #[test]
    fn tampered_ciphertext_fails_auth() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (ct, nonce) = c.encrypt("sensitive").unwrap();
        // Flip a single byte in the last 4 base64 characters (where the
        // AEAD tag lives). Using index math (not character replacement)
        // guarantees a mutation regardless of which chars the random
        // ciphertext happens to contain.
        let mut bytes = ct.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).unwrap();
        assert!(c.decrypt(&tampered, &nonce).is_err());
    }

    #[test]
    fn wrong_nonce_fails_decryption() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (ct, _nonce) = c.encrypt("secret").unwrap();
        let (_, other_nonce) = c.encrypt("other").unwrap();
        assert!(c.decrypt(&ct, &other_nonce).is_err());
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let c1 = FieldCipher::new(&test_key(), "v1");
        let (ct, nonce) = c1.encrypt("secret").unwrap();

        let mut other_key = test_key();
        other_key[0] ^= 0xFF;
        let c2 = FieldCipher::new(&other_key, "v1");
        assert!(c2.decrypt(&ct, &nonce).is_err());
    }

    #[test]
    fn invalid_base64_ciphertext_returns_error() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (_, nonce) = c.encrypt("ok").unwrap();
        let err = c.decrypt("not@base64!!!", &nonce).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn invalid_base64_nonce_returns_error() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (ct, _) = c.encrypt("ok").unwrap();
        assert!(c.decrypt(&ct, "@@@@").is_err());
    }

    #[test]
    fn nonce_wrong_length_returns_error() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (ct, _) = c.encrypt("ok").unwrap();
        // 11 bytes (not 12) base64-encoded.
        let short_nonce = B64.encode([1u8; 11]);
        let err = c.decrypt(&ct, &short_nonce).unwrap_err();
        match err {
            AppError::Internal(e) => assert!(e.to_string().contains("Nonce")),
            _ => panic!("expected Internal error, got {:?}", err),
        }
    }

    #[test]
    fn key_version_is_retained() {
        let c = FieldCipher::new(&test_key(), "v7");
        assert_eq!(c.key_version, "v7");
    }

    #[test]
    fn empty_plaintext_roundtrips() {
        let c = FieldCipher::new(&test_key(), "v1");
        let (ct, nonce) = c.encrypt("").unwrap();
        assert_eq!(c.decrypt(&ct, &nonce).unwrap(), "");
    }
}
