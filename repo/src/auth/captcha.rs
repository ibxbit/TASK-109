use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Claims embedded inside the captcha token (short-lived JWT).
#[derive(Debug, Serialize, Deserialize)]
struct CaptchaClaims {
    /// The correct numeric answer.
    answer: i32,
    /// Expiry as Unix timestamp (5 minutes from generation).
    exp: i64,
}

pub struct CaptchaChallenge {
    /// Human-readable question, e.g. "3 + 7 = ?"
    pub question: String,
    /// Signed JWT encoding the answer — returned to the client, who
    /// must echo it back together with their numeric answer.
    pub token: String,
}

/// Generate a random addition CAPTCHA signed with the server's JWT secret.
pub fn generate(jwt_secret: &str) -> CaptchaChallenge {
    let mut rng = rand::thread_rng();
    let a: i32 = rng.gen_range(1..=9);
    let b: i32 = rng.gen_range(1..=9);
    let answer = a + b;

    let exp = (chrono::Utc::now() + chrono::Duration::minutes(5)).timestamp();
    let claims = CaptchaClaims { answer, exp };

    // Failure here is a programming error (bad secret); unwrap is intentional.
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .expect("captcha JWT encoding failed");

    CaptchaChallenge {
        question: format!("{} + {} = ?", a, b),
        token,
    }
}

/// Verify that `answer` matches the value embedded in the signed `token`.
/// Returns false on wrong answer, expired token, or tampered signature.
pub fn verify(token: &str, answer: i32, jwt_secret: &str) -> bool {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    match decode::<CaptchaClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => data.claims.answer == answer,
        Err(_) => false,
    }
}
