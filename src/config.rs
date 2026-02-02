use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_api_port")]
    pub api_port: u16,

    pub api_key: Option<String>,

    #[serde(default = "default_chrome_path")]
    pub chrome_path: String,

    #[serde(default = "default_browser_pool_size")]
    pub browser_pool_size: usize,

    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    #[serde(default = "default_max_timeout")]
    pub max_timeout: u64,

    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,

    #[serde(default = "default_max_requests_per_page")]
    pub max_requests_per_page: usize,

    #[serde(default = "default_max_domains_per_page")]
    pub max_domains_per_page: usize,

    #[serde(default = "default_screenshot_dir")]
    pub screenshot_dir: PathBuf,
}

fn default_api_port() -> u16 { 14786 }
fn default_chrome_path() -> String { "/usr/bin/chromium".to_string() }
fn default_browser_pool_size() -> usize { 10 }
fn default_request_timeout() -> u64 { 30 }
fn default_max_timeout() -> u64 { 180 }
fn default_cache_ttl() -> u64 { 3600 }
fn default_max_requests_per_page() -> usize { 2000 }
fn default_max_domains_per_page() -> usize { 200 }
fn default_screenshot_dir() -> PathBuf { PathBuf::from("/app/screenshots") }

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let config = Config {
            api_port: std::env::var("API_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_api_port),
            api_key: std::env::var("API_KEY").ok(),
            chrome_path: std::env::var("CHROME_PATH")
                .unwrap_or_else(|_| default_chrome_path()),
            browser_pool_size: std::env::var("BROWSER_POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_browser_pool_size),
            request_timeout: std::env::var("REQUEST_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_request_timeout),
            max_timeout: std::env::var("MAX_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_max_timeout),
            cache_ttl: std::env::var("CACHE_TTL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_cache_ttl),
            max_requests_per_page: std::env::var("MAX_REQUESTS_PER_PAGE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_max_requests_per_page),
            max_domains_per_page: std::env::var("MAX_DOMAINS_PER_PAGE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(default_max_domains_per_page),
            screenshot_dir: std::env::var("SCREENSHOT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| default_screenshot_dir()),
        };

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            api_port: default_api_port(),
            api_key: None,
            chrome_path: default_chrome_path(),
            browser_pool_size: default_browser_pool_size(),
            request_timeout: default_request_timeout(),
            max_timeout: default_max_timeout(),
            cache_ttl: default_cache_ttl(),
            max_requests_per_page: default_max_requests_per_page(),
            max_domains_per_page: default_max_domains_per_page(),
            screenshot_dir: default_screenshot_dir(),
        }
    }
}
