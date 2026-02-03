use crate::config::Config;
use crate::error::{AppError, Result};
use crate::models::CrawlerOptions;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{CookieParam, SetCookiesParams};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 500;
const HEALTH_CHECK_TIMEOUT_MS: u64 = 5000;

pub struct BrowserPool {
    browser: Arc<RwLock<Option<Browser>>>,
    semaphore: Arc<Semaphore>,
    config: Config,
    is_healthy: Arc<AtomicBool>,
    recreation_count: Arc<AtomicU64>,
    recreation_lock: Arc<Mutex<()>>,
}

impl BrowserPool {
    pub async fn new(config: Config) -> Result<Self> {
        let pool = Self {
            browser: Arc::new(RwLock::new(None)),
            semaphore: Arc::new(Semaphore::new(config.browser_pool_size)),
            config,
            is_healthy: Arc::new(AtomicBool::new(false)),
            recreation_count: Arc::new(AtomicU64::new(0)),
            recreation_lock: Arc::new(Mutex::new(())),
        };

        pool.ensure_browser().await?;

        Ok(pool)
    }

    async fn create_browser(config: &Config) -> Result<Browser> {
        let browser_config = BrowserConfig::builder()
            .chrome_executable(&config.chrome_path)
            .no_sandbox()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-setuid-sandbox")
            .arg("--disable-extensions")
            .arg("--disable-background-networking")
            .arg("--disable-sync")
            .arg("--disable-translate")
            .arg("--hide-scrollbars")
            .arg("--metrics-recording-only")
            .arg("--mute-audio")
            .arg("--no-first-run")
            .arg("--safebrowsing-disable-auto-update")
            .arg("--ignore-certificate-errors")
            .arg("--ignore-ssl-errors")
            .arg("--ignore-certificate-errors-spki-list")
            .arg("--disable-features=IsolateOrigins,site-per-process")
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-web-security")
            .window_size(1920, 1080)
            .build()
            .map_err(|e| AppError::BrowserError(e.to_string()))?;

        let (browser, mut handler) = Browser::launch(browser_config)
            .await
            .map_err(|e| AppError::BrowserError(format!("Failed to launch browser: {}", e)))?;

        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                debug!("Browser event: {:?}", event);
            }
            debug!("Browser handler exited - browser connection closed");
        });

        info!("Browser launched successfully");
        Ok(browser)
    }

    async fn ensure_browser(&self) -> Result<()> {
        if self.is_healthy.load(Ordering::SeqCst) {
            let browser_guard = self.browser.read().await;
            if browser_guard.is_some() {
                return Ok(());
            }
        }

        let _lock = self.recreation_lock.lock().await;

        if self.is_healthy.load(Ordering::SeqCst) {
            let browser_guard = self.browser.read().await;
            if browser_guard.is_some() {
                return Ok(());
            }
        }

        info!("Creating new browser instance...");
        self.is_healthy.store(false, Ordering::SeqCst);

        let browser = Self::create_browser(&self.config).await?;

        {
            let mut browser_guard = self.browser.write().await;
            *browser_guard = Some(browser);
        }

        self.is_healthy.store(true, Ordering::SeqCst);
        self.recreation_count.fetch_add(1, Ordering::SeqCst);

        info!(
            "Browser instance created (total recreations: {})",
            self.recreation_count.load(Ordering::SeqCst)
        );

        Ok(())
    }

    async fn health_check(&self) -> bool {
        let browser_guard = self.browser.read().await;
        let browser = match browser_guard.as_ref() {
            Some(b) => b,
            None => return false,
        };

        let check = async {
            match browser.new_page("about:blank").await {
                Ok(page) => {
                    match page.evaluate("1+1").await {
                        Ok(_) => {
                            let _ = page.close().await;
                            true
                        }
                        Err(e) => {
                            warn!("Health check evaluate failed: {}", e);
                            false
                        }
                    }
                }
                Err(e) => {
                    warn!("Health check failed to create page: {}", e);
                    false
                }
            }
        };

        match tokio::time::timeout(Duration::from_millis(HEALTH_CHECK_TIMEOUT_MS), check).await {
            Ok(result) => result,
            Err(_) => {
                warn!("Health check timed out");
                false
            }
        }
    }

    pub async fn invalidate_browser(&self) {
        warn!("Invalidating current browser instance");
        self.is_healthy.store(false, Ordering::SeqCst);

        let mut browser_guard = self.browser.write().await;
        if let Some(browser) = browser_guard.take() {
            drop(browser);
        }
    }

    pub async fn get_page(&self, options: &CrawlerOptions) -> Result<Page> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| AppError::BrowserError(e.to_string()))?;

        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                warn!("Retry attempt {} for get_page", attempt);
                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            }

            if let Err(e) = self.ensure_browser().await {
                error!("Failed to ensure browser: {}", e);
                last_error = Some(e);
                continue;
            }

            if attempt > 0 || !self.is_healthy.load(Ordering::SeqCst) {
                if !self.health_check().await {
                    warn!("Browser health check failed, recreating...");
                    self.invalidate_browser().await;
                    continue;
                }
            }

            match self.try_get_page(options).await {
                Ok(page) => return Ok(page),
                Err(e) => {
                    if Self::is_connection_error(&e) {
                        warn!("Connection error getting page: {}, will retry", e);
                        self.invalidate_browser().await;
                        last_error = Some(e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AppError::BrowserError("Failed to get page after max retries".to_string())
        }))
    }

    async fn try_get_page(&self, options: &CrawlerOptions) -> Result<Page> {
        let browser_guard = self.browser.read().await;
        let browser = browser_guard.as_ref()
            .ok_or_else(|| AppError::BrowserError("Browser not initialized".to_string()))?;

        let page = tokio::time::timeout(
            Duration::from_millis(HEALTH_CHECK_TIMEOUT_MS),
            browser.new_page("about:blank")
        )
        .await
        .map_err(|_| AppError::BrowserError("Timeout creating page - browser connection may be dead".to_string()))?
        .map_err(|e| AppError::BrowserError(format!("Failed to create page: {}", e)))?;

        drop(browser_guard);

        let user_agent = options.user_agent.as_deref().unwrap_or(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        );
        tokio::time::timeout(
            Duration::from_millis(HEALTH_CHECK_TIMEOUT_MS),
            page.set_user_agent(user_agent)
        )
        .await
        .map_err(|_| AppError::BrowserError("Timeout setting user agent - browser connection may be dead".to_string()))?
        .map_err(|e| AppError::BrowserError(format!("Failed to set user agent: {}", e)))?;

        if let Some(ref cookies_str) = options.cookies {
            let cookies = Self::parse_cookies(cookies_str, &options.url);
            if !cookies.is_empty() {
                let params = SetCookiesParams::new(cookies);
                tokio::time::timeout(
                    Duration::from_millis(HEALTH_CHECK_TIMEOUT_MS),
                    page.execute(params)
                )
                .await
                .map_err(|_| AppError::BrowserError("Timeout setting cookies - browser connection may be dead".to_string()))?
                .map_err(|e| AppError::BrowserError(format!("Failed to set cookies: {}", e)))?;
            }
        }

        Ok(page)
    }

    pub async fn navigate_and_wait(
        &self,
        page: &Page,
        url: &str,
        options: &CrawlerOptions,
    ) -> Result<String> {
        let timeout = Duration::from_secs(options.timeout.unwrap_or(self.config.request_timeout));

        let result = tokio::time::timeout(timeout, async {
            page.goto(url)
                .await
                .map_err(|e| {
                    let err_str = e.to_string();
                    if Self::is_connection_error_str(&err_str) {
                        self.is_healthy.store(false, Ordering::SeqCst);
                    }
                    AppError::BrowserError(format!("Navigation failed: {}", e))
                })?;

            page.evaluate("document.readyState")
                .await
                .map_err(|e| {
                    let err_str = e.to_string();
                    if Self::is_connection_error_str(&err_str) {
                        self.is_healthy.store(false, Ordering::SeqCst);
                    }
                    AppError::BrowserError(format!("Ready state check failed: {}", e))
                })?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|_| AppError::Timeout(timeout.as_secs()))?;

        result?;

        if let Some(ref selector) = options.wait_for_selector {
            tokio::time::timeout(timeout, async {
                page.find_element(selector)
                    .await
                    .map_err(|e| AppError::BrowserError(format!("Selector wait failed: {}", e)))
            })
            .await
            .map_err(|_| AppError::Timeout(timeout.as_secs()))??;
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let html = page
            .content()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if Self::is_connection_error_str(&err_str) {
                    self.is_healthy.store(false, Ordering::SeqCst);
                }
                AppError::BrowserError(format!("Failed to get content: {}", e))
            })?;

        Ok(html)
    }

    pub async fn take_screenshot(
        &self,
        page: &Page,
        full_page: bool,
    ) -> Result<Vec<u8>> {
        let params = ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(full_page)
            .build();

        let screenshot = page
            .screenshot(params)
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if Self::is_connection_error_str(&err_str) {
                    self.is_healthy.store(false, Ordering::SeqCst);
                }
                AppError::ScreenshotError(e.to_string())
            })?;

        Ok(screenshot)
    }

    pub fn available_slots(&self) -> usize {
        self.semaphore.available_permits()
    }

    pub fn total_slots(&self) -> usize {
        self.config.browser_pool_size
    }

    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::SeqCst)
    }

    pub fn recreation_count(&self) -> u64 {
        self.recreation_count.load(Ordering::SeqCst)
    }

    pub fn is_connection_error(err: &AppError) -> bool {
        match err {
            AppError::BrowserError(msg) => Self::is_connection_error_str(msg),
            _ => false,
        }
    }

    fn is_connection_error_str(err_msg: &str) -> bool {
        let connection_error_patterns = [
            "AlreadyClosed",
            "Ws(AlreadyClosed)",
            "WebSocket",
            "connection",
            "Connection",
            "ConnectionClosed",
            "channel closed",
            "Channel closed",
            "browser closed",
            "Browser closed",
            "target closed",
            "Target closed",
            "session closed",
            "Session closed",
            "pipe",
            "Pipe",
            "disconnected",
            "Disconnected",
            "not connected",
            "Not connected",
            "may be dead",
            "Timeout creating page",
            "Timeout setting",
        ];

        let err_lower = err_msg.to_lowercase();
        connection_error_patterns.iter().any(|pattern| {
            err_lower.contains(&pattern.to_lowercase())
        })
    }

    fn parse_cookies(cookies_str: &str, url: &str) -> Vec<CookieParam> {
        let domain = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default();

        cookies_str
            .split(';')
            .filter_map(|cookie| {
                let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some(CookieParam::builder()
                        .name(parts[0].trim())
                        .value(parts[1].trim())
                        .domain(&domain)
                        .build()
                        .unwrap())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_detection() {
        assert!(BrowserPool::is_connection_error_str("Ws(AlreadyClosed)"));
        assert!(BrowserPool::is_connection_error_str("WS Connection error: Ws(AlreadyClosed)"));
        assert!(BrowserPool::is_connection_error_str("WebSocket connection failed"));
        assert!(BrowserPool::is_connection_error_str("connection reset"));
        assert!(BrowserPool::is_connection_error_str("Connection refused"));
        assert!(BrowserPool::is_connection_error_str("channel closed"));
        assert!(BrowserPool::is_connection_error_str("browser closed unexpectedly"));
        assert!(BrowserPool::is_connection_error_str("target closed"));
        assert!(BrowserPool::is_connection_error_str("session closed"));
        assert!(BrowserPool::is_connection_error_str("pipe broken"));
        assert!(BrowserPool::is_connection_error_str("client disconnected"));
        assert!(BrowserPool::is_connection_error_str("not connected to browser"));
        assert!(BrowserPool::is_connection_error_str("Timeout creating page - browser connection may be dead"));
        assert!(BrowserPool::is_connection_error_str("Timeout setting user agent - browser connection may be dead"));
        assert!(BrowserPool::is_connection_error_str("browser connection may be dead"));
        assert!(!BrowserPool::is_connection_error_str("timeout waiting for element"));
        assert!(!BrowserPool::is_connection_error_str("element not found"));
        assert!(!BrowserPool::is_connection_error_str("invalid selector"));
        assert!(!BrowserPool::is_connection_error_str("page load failed"));
        assert!(!BrowserPool::is_connection_error_str("JavaScript error"));
    }

    #[test]
    fn test_is_connection_error_with_app_error() {
        let err = AppError::BrowserError("Ws(AlreadyClosed)".to_string());
        assert!(BrowserPool::is_connection_error(&err));

        let err = AppError::BrowserError("element not found".to_string());
        assert!(!BrowserPool::is_connection_error(&err));

        let err = AppError::Timeout(30);
        assert!(!BrowserPool::is_connection_error(&err));

        let err = AppError::ScrapingError("parsing failed".to_string());
        assert!(!BrowserPool::is_connection_error(&err));
    }
}
