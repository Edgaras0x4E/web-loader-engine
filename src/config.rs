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

    #[serde(default = "default_user_agent")]
    pub default_user_agent: String,

    #[serde(default)]
    pub user_agent_pool: Vec<String>,

    #[serde(default = "default_user_agent_rotation")]
    pub user_agent_rotation: String,
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
fn default_user_agent() -> String {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string()
}
fn default_user_agent_rotation() -> String { "off".to_string() }

fn load_user_agent_pool() -> Vec<String> {
    let from_file = std::env::var("USER_AGENT_POOL_FILE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|path| std::fs::read_to_string(&path).ok());

    let raw = from_file.or_else(|| {
        std::env::var("USER_AGENT_POOL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    });

    raw.map(|text| parse_user_agent_pool(&text))
        .unwrap_or_default()
}

fn parse_user_agent_pool(text: &str) -> Vec<String> {
    text.split(|c| c == '\n' || c == '|')
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

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
            default_user_agent: std::env::var("DEFAULT_USER_AGENT")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(default_user_agent),
            user_agent_pool: load_user_agent_pool(),
            user_agent_rotation: std::env::var("USER_AGENT_ROTATION")
                .ok()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(default_user_agent_rotation),
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
            default_user_agent: default_user_agent(),
            user_agent_pool: Vec::new(),
            user_agent_rotation: default_user_agent_rotation(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_user_agent_pool;

    #[test]
    fn splits_on_pipe() {
        let pool = parse_user_agent_pool("UA1|UA2|UA3");
        assert_eq!(pool, vec!["UA1", "UA2", "UA3"]);
    }

    #[test]
    fn splits_on_newline() {
        let pool = parse_user_agent_pool("UA1\nUA2\nUA3");
        assert_eq!(pool, vec!["UA1", "UA2", "UA3"]);
    }

    #[test]
    fn splits_on_both_separators_mixed() {
        let pool = parse_user_agent_pool("UA1|UA2\nUA3|UA4");
        assert_eq!(pool, vec!["UA1", "UA2", "UA3", "UA4"]);
    }

    #[test]
    fn skips_comment_lines() {
        let pool = parse_user_agent_pool("UA1\n# comment\nUA2\n#another");
        assert_eq!(pool, vec!["UA1", "UA2"]);
    }

    #[test]
    fn skips_empty_lines_and_trims_whitespace() {
        let pool = parse_user_agent_pool("  UA1  \n\n   \n  UA2\t\n");
        assert_eq!(pool, vec!["UA1", "UA2"]);
    }

    #[test]
    fn empty_input_returns_empty_pool() {
        assert!(parse_user_agent_pool("").is_empty());
        assert!(parse_user_agent_pool("   \n\n   ").is_empty());
        assert!(parse_user_agent_pool("# only a comment").is_empty());
    }

    #[test]
    fn preserves_user_agent_content_with_internal_spaces() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0";
        let pool = parse_user_agent_pool(ua);
        assert_eq!(pool, vec![ua]);
    }
}
