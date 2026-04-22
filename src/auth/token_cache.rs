//! In-memory token cache with TTL expiration.
//!
//! Shared across all auth strategies for OAuth2 tokens.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct TokenEntry {
    pub token: String,
    pub expires_at: Instant,
}

#[derive(Clone)]
pub struct TokenCache {
    inner: Arc<Mutex<HashMap<String, TokenEntry>>>,
}

impl TokenCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let map = self.inner.lock().unwrap();
        map.get(key).and_then(|entry| {
            if entry.expires_at > Instant::now() {
                Some(entry.token.clone())
            } else {
                None
            }
        })
    }

    pub fn set(&self, key: &str, token: String, ttl_secs: u64) {
        let mut map = self.inner.lock().unwrap();
        map.insert(
            key.to_string(),
            TokenEntry {
                token,
                expires_at: Instant::now() + std::time::Duration::from_secs(ttl_secs),
            },
        );
    }
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let cache = TokenCache::new();
        cache.set("tool1", "tok_abc".to_string(), 300);
        assert_eq!(cache.get("tool1"), Some("tok_abc".to_string()));
        assert_eq!(cache.get("tool2"), None);
    }

    #[test]
    fn test_expired_token() {
        let cache = TokenCache::new();
        cache.set("tool1", "tok_abc".to_string(), 0);
        assert_eq!(cache.get("tool1"), None);
    }

    #[test]
    fn test_clone_shares_state() {
        let cache = TokenCache::new();
        cache.set("tool1", "tok_abc".to_string(), 300);
        let cloned = cache.clone();
        assert_eq!(cloned.get("tool1"), Some("tok_abc".to_string()));
    }
}
