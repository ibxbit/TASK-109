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
