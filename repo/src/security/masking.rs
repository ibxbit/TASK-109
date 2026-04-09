//! Identifier masking for structured logs.
//!
//! All user-identifiable IDs written to log fields must pass through
//! one of these helpers so that log aggregators never receive full
//! UUIDs or usernames.  Only the last 2 hex/printable characters are
//! retained, which is enough to correlate log lines within a session
//! without exposing the full identifier.

use uuid::Uuid;

/// Mask a UUID — show only the trailing 2 hex characters.
///
/// ```text
/// mask_id(&uuid) → "**…3f"
/// ```
pub fn mask_id(id: &Uuid) -> String {
    let s = id.as_simple().to_string(); // 32 hex chars, no dashes
    format!("**\u{2026}{}", &s[s.len() - 2..])
}

/// Mask an arbitrary string — show only the trailing 2 characters.
///
/// If the string is 2 characters or shorter the whole value is
/// replaced with asterisks to avoid trivial recovery.
pub fn mask_str(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= 2 {
        return "*".repeat(chars.len());
    }
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("**\u{2026}{}", tail)
}

/// Convenience wrapper for usernames.
#[inline]
pub fn mask_username(username: &str) -> String {
    mask_str(username)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_id_keeps_last_two() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let m = mask_id(&id);
        assert!(m.ends_with("00"), "got: {}", m);
        assert!(m.starts_with("**"), "got: {}", m);
    }

    #[test]
    fn mask_str_short() {
        assert_eq!(mask_str("ab"), "**");
        assert_eq!(mask_str("a"), "*");
    }

    #[test]
    fn mask_str_normal() {
        let m = mask_str("hello");
        assert!(m.ends_with("lo"), "got: {}", m);
    }
}
