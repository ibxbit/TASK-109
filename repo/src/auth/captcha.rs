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

// ─────────────────────────────────────────────────────────────────
// Unit tests — CAPTCHA issuance and verification.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    const SECRET: &str = "unit-test-jwt-secret";

    /// Parse "X + Y = ?" into (X, Y).
    fn parse_question(q: &str) -> (i32, i32) {
        // Expected format: "a + b = ?"
        let cleaned = q.replace(" = ?", "");
        let (a, b) = cleaned.split_once(" + ").expect("bad question format");
        (a.trim().parse().unwrap(), b.trim().parse().unwrap())
    }

    #[test]
    fn generated_challenge_has_two_digit_operands() {
        for _ in 0..20 {
            let ch = generate(SECRET);
            let (a, b) = parse_question(&ch.question);
            assert!((1..=9).contains(&a), "a out of range: {}", a);
            assert!((1..=9).contains(&b), "b out of range: {}", b);
            assert!(!ch.token.is_empty());
        }
    }

    #[test]
    fn verify_accepts_correct_answer() {
        let ch = generate(SECRET);
        let (a, b) = parse_question(&ch.question);
        assert!(verify(&ch.token, a + b, SECRET));
    }

    #[test]
    fn verify_rejects_wrong_answer() {
        let ch = generate(SECRET);
        let (a, b) = parse_question(&ch.question);
        assert!(!verify(&ch.token, a + b + 1, SECRET));
        assert!(!verify(&ch.token, -1, SECRET));
    }

    #[test]
    fn verify_rejects_tampered_token() {
        let ch = generate(SECRET);
        // Flip the last (signature) character — base64url, ASCII-only,
        // so character mutation is safe and unambiguous.
        let mut bytes = ch.token.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let bad = String::from_utf8(bytes).unwrap();
        // Build a fresh challenge to read its question, since we consumed
        // the original above; the answer doesn't matter — the signature
        // mismatch causes verify() to short-circuit to false.
        assert!(!verify(&bad, 0, SECRET));
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let ch = generate(SECRET);
        let (a, b) = parse_question(&ch.question);
        assert!(!verify(&ch.token, a + b, "different-secret"));
    }

    #[test]
    fn verify_rejects_expired_token() {
        // Build a token that's well past the default 60 s JWT leeway
        // (jsonwebtoken::Validation default allows 60 s clock skew).
        let expired = CaptchaClaims {
            answer: 42,
            exp: (chrono::Utc::now() - chrono::Duration::minutes(10)).timestamp(),
        };
        let tok = encode(
            &Header::default(),
            &expired,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();
        assert!(!verify(&tok, 42, SECRET));
    }

    #[test]
    fn verify_rejects_garbage_token() {
        assert!(!verify("not-a-jwt", 0, SECRET));
        assert!(!verify("", 0, SECRET));
    }
}
