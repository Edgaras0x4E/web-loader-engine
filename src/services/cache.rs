use crate::models::LoadResponse;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use tracing::debug;

struct CacheEntry {
    response: LoadResponse,
    created_at: Instant,
    ttl: Duration,
}

pub struct CacheService {
    cache: DashMap<String, CacheEntry>,
    default_ttl: Duration,
}

impl CacheService {
    pub fn new(default_ttl_secs: u64) -> Self {
        Self {
            cache: DashMap::new(),
            default_ttl: Duration::from_secs(default_ttl_secs),
        }
    }

    pub fn get(&self, key: &str) -> Option<LoadResponse> {
        if let Some(entry) = self.cache.get(key) {
            if entry.created_at.elapsed() < entry.ttl {
                debug!("Cache hit for {}", key);
                let mut response = entry.response.clone();
                response.metadata.cached = true;
                return Some(response);
            } else {
                debug!("Cache expired for {}", key);
                drop(entry);
                self.cache.remove(key);
            }
        }
        debug!("Cache miss for {}", key);
        None
    }

    pub fn get_with_tolerance(&self, key: &str, tolerance_secs: Option<u64>) -> Option<LoadResponse> {
        if let Some(entry) = self.cache.get(key) {
            let max_age = tolerance_secs
                .map(Duration::from_secs)
                .unwrap_or(entry.ttl);

            if entry.created_at.elapsed() < max_age {
                debug!("Cache hit for {} (tolerance: {:?})", key, tolerance_secs);
                let mut response = entry.response.clone();
                response.metadata.cached = true;
                return Some(response);
            }
        }
        None
    }

    pub fn set(&self, key: String, response: LoadResponse, ttl_secs: Option<u64>) {
        let ttl = ttl_secs
            .map(Duration::from_secs)
            .unwrap_or(self.default_ttl);

        debug!("Caching response for {} (TTL: {:?})", key, ttl);

        self.cache.insert(key, CacheEntry {
            response,
            created_at: Instant::now(),
            ttl,
        });
    }

    pub fn invalidate(&self, key: &str) {
        self.cache.remove(key);
    }

    pub fn clear(&self) {
        self.cache.clear();
    }

    pub fn cleanup_expired(&self) -> usize {
        let mut removed = 0;
        self.cache.retain(|_, entry| {
            let keep = entry.created_at.elapsed() < entry.ttl;
            if !keep {
                removed += 1;
            }
            keep
        });
        debug!("Cache cleanup: removed {} expired entries", removed);
        removed
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }

    pub fn generate_cache_key(url: &str, options_hash: u64) -> String {
        format!("{}:{}", url, options_hash)
    }
}

impl Default for CacheService {
    fn default() -> Self {
        Self::new(3600)
    }
}
