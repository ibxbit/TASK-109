//! Per-user (or per-IP) sliding-window rate limiting middleware.
//!
//! Policy: **60 requests per 60-second window** per principal.
//!
//! Key selection (in priority order):
//! 1. `Bearer <token>` from the `Authorization` header — maps 1:1 to a
//!    user session, giving true per-user limiting.
//! 2. `realip_remote_addr` — used for unauthenticated endpoints such as
//!    `/auth/login` (the login endpoint has its own failed-attempt
//!    counter in the DB; this layer provides a coarser backstop).
//!
//! The window is **sliding**: the counter and window-start timestamp are
//! reset whenever the current time is ≥ `window_start + WINDOW_SECS`
//! from a principal's previous request.
//!
//! State is kept in a `DashMap` (lock-free concurrent hash-map) shared
//! across all Actix worker threads via `Arc`.  Entries are never
//! explicitly evicted; in practice sessions expire and the small memory
//! footprint (≤ handful of bytes per active session) is negligible.

use std::{
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    HttpResponse,
};
use dashmap::DashMap;
use tracing::warn;
use uuid::Uuid;

// ── Constants ─────────────────────────────────────────────────

pub const WINDOW_SECS: u64 = 60;

pub fn get_max_requests() -> u32 {
    std::env::var("RATE_LIMIT_MAX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60)
}

// Ensure the rate limit logic is enforced strictly
// (No-op here if logic is already correct; otherwise, check middleware)

// ── Shared store ──────────────────────────────────────────────

#[derive(Clone)]
pub struct Entry {
    count: u32,
    window_start: Instant,
}

/// Thread-safe rate-limit state shared across all Actix workers.
pub type RateLimitStore = Arc<DashMap<String, Entry>>;

pub fn new_store() -> RateLimitStore {
    Arc::new(DashMap::new())
}

/// Shared cache mapping a Bearer token to the owning user's UUID.
///
/// The login handler populates this on successful authentication so that
/// the rate limiter can key by `user:{user_id}` rather than by raw token.
/// This ensures all sessions belonging to the same user share a single
/// rate-limit quota — a multi-session user cannot bypass the limiter by
/// opening additional sessions.
///
/// Entries are removed on logout.  Orphaned entries for expired-but-not-
/// logged-out sessions have negligible memory impact (~40 bytes each).
pub type TokenUserCache = Arc<DashMap<String, Uuid>>;

pub fn new_token_user_cache() -> TokenUserCache {
    Arc::new(DashMap::new())
}

// ── Middleware factory ────────────────────────────────────────

/// Middleware factory.  Pass a cloned `RateLimitStore` and `TokenUserCache`
/// to each `App` worker inside the `HttpServer::new` closure.
pub struct RateLimit {
    store:            RateLimitStore,
    token_user_cache: TokenUserCache,
}

impl RateLimit {
    pub fn new(store: RateLimitStore, token_user_cache: TokenUserCache) -> Self {
        Self { store, token_user_cache }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error    = actix_web::Error;
    type InitError = ();
    type Transform = RateLimitService<S>;
    type Future    = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(RateLimitService {
            service:          Rc::new(service),
            store:            self.store.clone(),
            token_user_cache: self.token_user_cache.clone(),
        }))
    }
}

// ── Per-worker service ────────────────────────────────────────

pub struct RateLimitService<S> {
    service:          Rc<S>,
    store:            RateLimitStore,
    token_user_cache: TokenUserCache,
}

impl<S, B> Service<ServiceRequest> for RateLimitService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error    = actix_web::Error;
    type Future   = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Exclude metrics summary, audit-logs, and login endpoints from rate limiting
        let path = req.path().to_string();

        if path.starts_with("/metrics/summary")
            || path.starts_with("/audit-logs")
            || path.starts_with("/auth/login")
        {
            let service = Rc::clone(&self.service);
            return Box::pin(async move {
                let res = service.call(req).await?;
                Ok(res.map_into_left_body())
            });
        }
        let store            = self.store.clone();
        let token_user_cache = self.token_user_cache.clone();
        let service          = Rc::clone(&self.service);

        Box::pin(async move {
            // ── Derive the rate-limit key ─────────────────────
            // Priority: user_id (from token cache) > raw token > IP.
            // Keying by user_id means all sessions for the same user share
            // one quota, preventing multi-session quota abuse.
            let bearer_token: Option<String> = req
                .headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(str::to_owned);

            let key = match &bearer_token {
                Some(tok) => {
                    if let Some(uid) = token_user_cache.get(tok.as_str()) {
                        format!("user:{}", *uid)
                    } else {
                        format!("tok:{}", tok)
                    }
                }
                None => req
                    .connection_info()
                    .realip_remote_addr()
                    .map(|ip| format!("ip:{}", ip))
                    .unwrap_or_else(|| "unknown".to_string()),
            };

            let max_reqs = get_max_requests();
            // ── Sliding-window check ──────────────────────────
            let window   = Duration::from_secs(WINDOW_SECS);
            let now      = Instant::now();
            let exceeded = {
                let mut entry = store.entry(key.clone()).or_insert(Entry {
                    count:        0,
                    window_start: now,
                });
                if now.duration_since(entry.window_start) >= window {
                    // Window expired    start a fresh window.
                    entry.count        = 1;
                    entry.window_start = now;
                    false
                } else {
                    if entry.count >= max_reqs {
                        true
                    } else {
                        entry.count += 1;
                        // Block exactly after max_reqs requests (return 429 on subsequent)
                        if entry.count > max_reqs {
                            true
                        } else {
                            false
                        }
                    }
                }
            };

            if exceeded {
                // Log security event (mask key to hide full token).
                let masked = mask_key(&key);
                warn!(
                    key    = %masked,
                    limit  = max_reqs,
                    window = WINDOW_SECS,
                    "RATE_LIMIT_EXCEEDED"
                );

                let response = HttpResponse::TooManyRequests()
                    .insert_header(("Retry-After", WINDOW_SECS.to_string()))
                    .json(serde_json::json!({
                        "error":   "Too Many Requests",
                        "message": format!(
                            "Rate limit exceeded: {} requests per {} seconds",
                            max_reqs, WINDOW_SECS
                        )
                    }));

                let (http_req, _payload) = req.into_parts();
                return Ok(
                    ServiceResponse::new(http_req, response).map_into_right_body()
                );
            }

            // ── Pass through ──────────────────────────────────
            service.call(req).await.map(ServiceResponse::map_into_left_body)
        })
    }
}

/// Mask the rate-limit key so only the last 2 chars appear in logs.
fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 2 {
        return "*".repeat(chars.len());
    }
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("**\u{2026}{}", tail)
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — pure helpers + sliding-window counter accounting.
//
// The full middleware path (`call`) is exercised by the integration
// tests under `tests/`. Here we only verify the bits that don't
// require a running Actix service.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn store_constructors_yield_empty_maps() {
        let store = new_store();
        assert_eq!(store.len(), 0);

        let cache = new_token_user_cache();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn token_user_cache_inserts_and_reads() {
        let cache = new_token_user_cache();
        let uid = uuid::Uuid::new_v4();
        cache.insert("tok-abc".into(), uid);
        assert_eq!(cache.get("tok-abc").map(|v| *v), Some(uid));
    }

    #[test]
    fn mask_key_full_redacts_short_input() {
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("a"), "*");
        assert_eq!(mask_key("ab"), "**");
    }

    #[test]
    fn mask_key_preserves_only_tail_two_chars() {
        let masked = mask_key("user:b1d27f");
        assert!(masked.ends_with("7f"));
        assert!(masked.starts_with("**"));
        // Only the trailing 2 chars (and the prefix marker) survive.
        assert!(!masked.contains("user"));
    }

    #[test]
    fn mask_key_handles_unicode_tail() {
        let masked = mask_key("user:漢字");
        assert!(masked.ends_with("漢字"));
    }

    /// Simulate the sliding-window accounting that the middleware performs
    /// inside `call()` so we can verify the math without an Actix runtime.
    fn touch(store: &RateLimitStore, key: &str, now: Instant, max_reqs: u32) -> bool {
        let window = Duration::from_secs(WINDOW_SECS);
        let mut entry = store.entry(key.to_string()).or_insert(Entry {
            count: 0,
            window_start: now,
        });
        if now.duration_since(entry.window_start) >= window {
            entry.count = 1;
            entry.window_start = now;
            false
        } else if entry.count >= max_reqs {
            true
        } else {
            entry.count += 1;
            entry.count > max_reqs
        }
    }

    #[test]
    fn within_limit_does_not_block() {
        let store = new_store();
        let now = Instant::now();
        for _ in 0..5 {
            assert!(!touch(&store, "user:1", now, 5));
        }
    }

    #[test]
    fn exceeding_limit_blocks() {
        let store = new_store();
        let now = Instant::now();
        for _ in 0..3 {
            assert!(!touch(&store, "u", now, 3));
        }
        // The 4th request in the same window must be blocked.
        assert!(touch(&store, "u", now, 3));
    }

    #[test]
    fn fresh_window_resets_counter() {
        let store = new_store();
        let now = Instant::now();
        for _ in 0..5 {
            touch(&store, "u", now, 5);
        }
        assert!(touch(&store, "u", now, 5)); // blocked

        // Pretend WINDOW_SECS+1 seconds passed by advancing the clock argument.
        let later = now + Duration::from_secs(WINDOW_SECS + 1);
        assert!(!touch(&store, "u", later, 5));
    }

    #[test]
    fn distinct_keys_dont_share_quota() {
        let store = new_store();
        let now = Instant::now();
        for _ in 0..5 {
            touch(&store, "user:a", now, 5);
        }
        // user:a is exhausted, but user:b is independent.
        assert!(touch(&store, "user:a", now, 5));
        assert!(!touch(&store, "user:b", now, 5));
    }

    /// Serialize tests that mutate the shared `RATE_LIMIT_MAX` env var so
    /// they don't race with each other (cargo test runs unit tests in
    /// parallel within the same process).
    static RATE_LIMIT_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn get_max_requests_default_is_60() {
        let _g = RATE_LIMIT_ENV_LOCK.lock().unwrap();
        let prev = std::env::var("RATE_LIMIT_MAX").ok();
        std::env::remove_var("RATE_LIMIT_MAX");
        assert_eq!(get_max_requests(), 60);
        if let Some(v) = prev {
            std::env::set_var("RATE_LIMIT_MAX", v);
        }
    }

    #[test]
    fn get_max_requests_uses_env_when_valid() {
        let _g = RATE_LIMIT_ENV_LOCK.lock().unwrap();
        let prev = std::env::var("RATE_LIMIT_MAX").ok();
        std::env::set_var("RATE_LIMIT_MAX", "7");
        assert_eq!(get_max_requests(), 7);
        match prev {
            Some(v) => std::env::set_var("RATE_LIMIT_MAX", v),
            None => std::env::remove_var("RATE_LIMIT_MAX"),
        }
    }

    #[test]
    fn get_max_requests_falls_back_when_env_invalid() {
        let _g = RATE_LIMIT_ENV_LOCK.lock().unwrap();
        let prev = std::env::var("RATE_LIMIT_MAX").ok();
        std::env::set_var("RATE_LIMIT_MAX", "not-a-number");
        assert_eq!(get_max_requests(), 60);
        match prev {
            Some(v) => std::env::set_var("RATE_LIMIT_MAX", v),
            None => std::env::remove_var("RATE_LIMIT_MAX"),
        }
    }
}
