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
