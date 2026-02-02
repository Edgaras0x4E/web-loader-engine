use crate::config::Config;
use crate::error::{AppError, Result};
use crate::models::CrawlerOptions;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{CookieParam, SetCookiesParams};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info};

pub struct BrowserPool {
    browser: Arc<Mutex<Option<Browser>>>,
    semaphore: Arc<Semaphore>,
    config: Config,
}

impl BrowserPool {
    pub async fn new(config: Config) -> Result<Self> {
        let browser = Self::create_browser(&config).await?;

        Ok(Self {
            browser: Arc::new(Mutex::new(Some(browser))),
            semaphore: Arc::new(Semaphore::new(config.browser_pool_size)),
            config,
        })
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
            .map_err(|e| AppError::BrowserError(e.to_string()))?;

        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                debug!("Browser event: {:?}", event);
            }
        });

        info!("Browser launched successfully");
        Ok(browser)
    }

    pub async fn get_page(&self, options: &CrawlerOptions) -> Result<Page> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| AppError::BrowserError(e.to_string()))?;

        let browser_guard = self.browser.lock().await;
        let browser = browser_guard.as_ref()
            .ok_or_else(|| AppError::BrowserError("Browser not initialized".to_string()))?;

        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| AppError::BrowserError(e.to_string()))?;

        drop(browser_guard);

        if let Some(ref ua) = options.user_agent {
            page.set_user_agent(ua)
                .await
                .map_err(|e| AppError::BrowserError(e.to_string()))?;
        } else {
            page.set_user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            ).await.map_err(|e| AppError::BrowserError(e.to_string()))?;
        }

        if let Some(ref cookies_str) = options.cookies {
            let cookies = Self::parse_cookies(cookies_str, &options.url);
            if !cookies.is_empty() {
                let params = SetCookiesParams::new(cookies);
                page.execute(params)
                    .await
                    .map_err(|e| AppError::BrowserError(e.to_string()))?;
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

        tokio::time::timeout(timeout, async {
            page.goto(url)
                .await
                .map_err(|e| AppError::BrowserError(format!("Navigation failed: {}", e)))?;

            page.evaluate("document.readyState")
                .await
                .map_err(|e| AppError::BrowserError(format!("Ready state check failed: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|_| AppError::Timeout(timeout.as_secs()))??;

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
            .map_err(|e| AppError::BrowserError(format!("Failed to get content: {}", e)))?;

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
            .map_err(|e| AppError::ScreenshotError(e.to_string()))?;

        Ok(screenshot)
    }

    pub fn available_slots(&self) -> usize {
        self.semaphore.available_permits()
    }

    pub fn total_slots(&self) -> usize {
        self.config.browser_pool_size
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
