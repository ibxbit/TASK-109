use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

/// Hash a plaintext password with Argon2id. Returns a PHC-formatted string.
#[allow(dead_code)]
pub fn hash(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default(); // Argon2id, recommended parameters
    Ok(argon2.hash_password(password.as_bytes(), &salt)?.to_string())
}

/// Verify a plaintext password against a stored PHC hash.
/// Returns false on mismatch or parse error (never panics).
pub fn verify(password: &str, phc_hash: &str) -> bool {
    let parsed = match PasswordHash::new(phc_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — Argon2 hash/verify roundtrip.
//
// `Argon2::default()` uses the OWASP-recommended memory-hard params,
// so each test takes ~50–100 ms. Keeping the number of tests small.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_succeeds() {
        let phc = hash("correct-horse-battery-staple").expect("hash");
        assert!(phc.starts_with("$argon2"));
        assert!(verify("correct-horse-battery-staple", &phc));
    }

    #[test]
    fn verify_wrong_password_returns_false() {
        let phc = hash("secret").expect("hash");
        assert!(!verify("not-secret", &phc));
    }

    #[test]
    fn verify_malformed_hash_returns_false_without_panic() {
        // Must not panic on garbage input — security invariant.
        assert!(!verify("anything", ""));
        assert!(!verify("anything", "definitely not a PHC string"));
        assert!(!verify("anything", "$argon2id$v=19$bad"));
    }

    #[test]
    fn distinct_hashes_for_same_password() {
        // Each call generates a fresh salt ⇒ hashes must differ.
        let a = hash("same").unwrap();
        let b = hash("same").unwrap();
        assert_ne!(a, b);
        // Both must still verify.
        assert!(verify("same", &a));
        assert!(verify("same", &b));
    }
}
