use std::env;

use base64::{engine::general_purpose::STANDARD as B64, Engine};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    /// 32-byte AES-256 key decoded from FIELD_ENCRYPTION_KEY (base64).
    pub field_encryption_key: [u8; 32],
    /// Directory for analytics export files (default: ./exports).
    pub exports_dir: String,
    /// Version label for the active encryption key (default: "v1").
    /// Increment this (e.g. "v2") after rotating FIELD_ENCRYPTION_KEY
    /// and re-encrypting all sensitive fields.
    pub encryption_key_version: String,
    /// Secret used to verify HMAC-SHA256 signatures on privileged endpoints.
    /// Set via HMAC_SECRET env var.
    pub hmac_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let key_b64 = env::var("FIELD_ENCRYPTION_KEY")
            .expect("FIELD_ENCRYPTION_KEY must be set");
        let key_bytes = B64
            .decode(&key_b64)
            .expect("FIELD_ENCRYPTION_KEY must be valid base64");
        if key_bytes.len() != 32 {
            panic!(
                "FIELD_ENCRYPTION_KEY must decode to exactly 32 bytes (got {})",
                key_bytes.len()
            );
        }
        let mut field_encryption_key = [0u8; 32];
        field_encryption_key.copy_from_slice(&key_bytes);

        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a valid number"),
            field_encryption_key,
            exports_dir: env::var("EXPORTS_DIR")
                .unwrap_or_else(|_| "./exports".to_string()),
            encryption_key_version: env::var("ENCRYPTION_KEY_VERSION")
                .unwrap_or_else(|_| "v1".to_string()),
            hmac_secret: env::var("HMAC_SECRET")
                .expect("HMAC_SECRET must be set"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — env parsing happy-path + panic paths.
//
// `from_env()` reads from process-wide env vars. To avoid races
// across parallel tests we serialise the env-touching tests
// behind a Mutex and restore the previous values on exit.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// 32-byte key, base64-encoded.
    fn valid_key_b64() -> String {
        B64.encode([7u8; 32])
    }

    /// Snapshot/restore guard for env vars touched by AppConfig.
    struct EnvScope {
        previous: Vec<(String, Option<String>)>,
    }

    impl EnvScope {
        fn new(vars: &[(&str, Option<&str>)]) -> Self {
            let previous: Vec<_> = vars
                .iter()
                .map(|(k, _)| (k.to_string(), env::var(k).ok()))
                .collect();
            for (k, v) in vars {
                match v {
                    Some(val) => env::set_var(k, val),
                    None => env::remove_var(k),
                }
            }
            Self { previous }
        }
    }

    impl Drop for EnvScope {
        fn drop(&mut self) {
            for (k, prev) in &self.previous {
                match prev {
                    Some(v) => env::set_var(k, v),
                    None => env::remove_var(k),
                }
            }
        }
    }

    /// Acquire `ENV_LOCK` recovering from poisoning — a prior `should_panic`
    /// test will poison the mutex, but that's fine: panic is exactly what
    /// those tests verify, and the guarded env state is fully restored by
    /// each test's `EnvScope` drop.
    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    #[test]
    fn from_env_parses_full_environment() {
        let _g = lock_env();
        let key = valid_key_b64();
        let _scope = EnvScope::new(&[
            ("DATABASE_URL",            Some("postgres://localhost/test")),
            ("JWT_SECRET",              Some("jwt")),
            ("HOST",                    Some("127.0.0.1")),
            ("PORT",                    Some("9090")),
            ("FIELD_ENCRYPTION_KEY",    Some(&key)),
            ("EXPORTS_DIR",             Some("/tmp/exports")),
            ("ENCRYPTION_KEY_VERSION",  Some("v3")),
            ("HMAC_SECRET",             Some("hmac-secret")),
        ]);

        let cfg = AppConfig::from_env();
        assert_eq!(cfg.database_url, "postgres://localhost/test");
        assert_eq!(cfg.jwt_secret, "jwt");
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 9090);
        assert_eq!(cfg.exports_dir, "/tmp/exports");
        assert_eq!(cfg.encryption_key_version, "v3");
        assert_eq!(cfg.hmac_secret, "hmac-secret");
        assert_eq!(cfg.field_encryption_key, [7u8; 32]);
    }

    #[test]
    fn from_env_uses_defaults_for_optional_vars() {
        let _g = lock_env();
        let key = valid_key_b64();
        let _scope = EnvScope::new(&[
            ("DATABASE_URL",            Some("postgres://x/y")),
            ("JWT_SECRET",              Some("j")),
            ("HOST",                    None),  // → default 0.0.0.0
            ("PORT",                    None),  // → default 8080
            ("FIELD_ENCRYPTION_KEY",    Some(&key)),
            ("EXPORTS_DIR",             None),  // → ./exports
            ("ENCRYPTION_KEY_VERSION",  None),  // → v1
            ("HMAC_SECRET",             Some("h")),
        ]);

        let cfg = AppConfig::from_env();
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.exports_dir, "./exports");
        assert_eq!(cfg.encryption_key_version, "v1");
    }

    #[test]
    #[should_panic(expected = "FIELD_ENCRYPTION_KEY must decode to exactly 32 bytes")]
    fn from_env_panics_on_short_key() {
        let _g = lock_env();
        let short_key = B64.encode([0u8; 16]);
        let _scope = EnvScope::new(&[
            ("DATABASE_URL",         Some("x")),
            ("JWT_SECRET",           Some("x")),
            ("FIELD_ENCRYPTION_KEY", Some(&short_key)),
            ("HMAC_SECRET",          Some("x")),
            ("PORT",                 None),
            ("HOST",                 None),
        ]);
        let _ = AppConfig::from_env();
    }

    #[test]
    #[should_panic(expected = "PORT must be a valid number")]
    fn from_env_panics_on_invalid_port() {
        let _g = lock_env();
        let key = valid_key_b64();
        let _scope = EnvScope::new(&[
            ("DATABASE_URL",         Some("x")),
            ("JWT_SECRET",           Some("x")),
            ("FIELD_ENCRYPTION_KEY", Some(&key)),
            ("HMAC_SECRET",          Some("x")),
            ("PORT",                 Some("abc")),
            ("HOST",                 None),
        ]);
        let _ = AppConfig::from_env();
    }

    #[test]
    #[should_panic(expected = "FIELD_ENCRYPTION_KEY must be valid base64")]
    fn from_env_panics_on_invalid_base64_key() {
        let _g = lock_env();
        let _scope = EnvScope::new(&[
            ("DATABASE_URL",         Some("x")),
            ("JWT_SECRET",           Some("x")),
            ("FIELD_ENCRYPTION_KEY", Some("not@@base64@@")),
            ("HMAC_SECRET",          Some("x")),
            ("PORT",                 None),
            ("HOST",                 None),
        ]);
        let _ = AppConfig::from_env();
    }
}
