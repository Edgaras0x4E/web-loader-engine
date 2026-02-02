use crate::config::Config;
use crate::error::{AppError, Result};
use dashmap::DashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tracing::warn;
use url::Url;

struct CircuitBreakerState {
    failures: usize,
    last_failure: Instant,
    open_until: Option<Instant>,
}

struct RateLimitState {
    requests: usize,
    window_start: Instant,
}

pub struct SecurityService {
    config: Config,
    circuit_breakers: DashMap<String, CircuitBreakerState>,
    rate_limits: DashMap<String, RateLimitState>,
    blocked_domains: Vec<String>,
}

impl SecurityService {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            circuit_breakers: DashMap::new(),
            rate_limits: DashMap::new(),
            blocked_domains: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "0.0.0.0".to_string(),
                "::1".to_string(),
            ],
        }
    }

    pub fn validate_url(&self, url_str: &str) -> Result<Url> {
        let url = Url::parse(url_str)
            .map_err(|e| AppError::InvalidUrl(format!("Invalid URL format: {}", e)))?;

        if !["http", "https"].contains(&url.scheme()) {
            return Err(AppError::InvalidUrl(format!(
                "Invalid scheme: {}. Only http and https are allowed",
                url.scheme()
            )));
        }

        if let Some(host) = url.host_str() {
            if self.is_blocked_host(host) {
                return Err(AppError::BlockedUrl(format!(
                    "Access to {} is not allowed",
                    host
                )));
            }

            if self.is_localhost_ip(host) {
                return Err(AppError::BlockedUrl(
                    "Access to localhost/internal IPs is not allowed".to_string()
                ));
            }
        } else {
            return Err(AppError::InvalidUrl("URL must have a host".to_string()));
        }

        if let Some(host) = url.host_str() {
            if !host.contains('.') && !self.blocked_domains.contains(&host.to_string()) {
                return Err(AppError::InvalidUrl(
                    "URL must have a valid TLD".to_string()
                ));
            }
        }

        Ok(url)
    }

    fn is_blocked_host(&self, host: &str) -> bool {
        let host_lower = host.to_lowercase();
        self.blocked_domains.iter().any(|blocked| {
            host_lower == *blocked || host_lower.ends_with(&format!(".{}", blocked))
        })
    }

    fn is_localhost_ip(&self, host: &str) -> bool {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return match ip {
                IpAddr::V4(ipv4) => {
                    ipv4.is_loopback() ||
                    ipv4.is_private() ||
                    ipv4.is_link_local() ||
                    ipv4.octets()[0] == 127
                }
                IpAddr::V6(ipv6) => {
                    ipv6.is_loopback()
                }
            };
        }

        let patterns = ["127.", "192.168.", "10.", "172.16.", "169.254."];
        patterns.iter().any(|p| host.starts_with(p))
    }

    pub fn check_circuit_breaker(&self, domain: &str) -> Result<()> {
        if let Some(state) = self.circuit_breakers.get(domain) {
            if let Some(open_until) = state.open_until {
                if Instant::now() < open_until {
                    warn!("Circuit breaker open for domain: {}", domain);
                    return Err(AppError::CircuitBreakerOpen(domain.to_string()));
                }
            }
        }
        Ok(())
    }

    pub fn record_failure(&self, domain: &str) {
        let mut entry = self.circuit_breakers.entry(domain.to_string())
            .or_insert(CircuitBreakerState {
                failures: 0,
                last_failure: Instant::now(),
                open_until: None,
            });

        entry.failures += 1;
        entry.last_failure = Instant::now();

        if entry.failures >= 5 {
            entry.open_until = Some(Instant::now() + Duration::from_secs(60));
            warn!("Circuit breaker opened for domain: {} (failures: {})", domain, entry.failures);
        }
    }

    pub fn record_success(&self, domain: &str) {
        if let Some(mut entry) = self.circuit_breakers.get_mut(domain) {
            entry.failures = 0;
            entry.open_until = None;
        }
    }

    pub fn check_rate_limit(&self, domain: &str) -> Result<()> {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let max_requests = self.config.max_requests_per_page;

        let mut entry = self.rate_limits.entry(domain.to_string())
            .or_insert(RateLimitState {
                requests: 0,
                window_start: now,
            });

        if now.duration_since(entry.window_start) > window {
            entry.requests = 0;
            entry.window_start = now;
        }

        entry.requests += 1;

        if entry.requests > max_requests {
            warn!("Rate limit exceeded for domain: {}", domain);
            return Err(AppError::RateLimitExceeded(domain.to_string()));
        }

        Ok(())
    }

    pub fn check_domain_count(&self, domains: &[String]) -> Result<()> {
        if domains.len() > self.config.max_domains_per_page {
            return Err(AppError::TooManyDomains(domains.len()));
        }
        Ok(())
    }

    pub fn extract_domain(url: &Url) -> String {
        url.host_str().unwrap_or("unknown").to_string()
    }
}

impl Default for SecurityService {
    fn default() -> Self {
        Self::new(Config::default())
    }
}
