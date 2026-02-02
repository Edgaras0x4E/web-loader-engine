use crate::config::Config;
use crate::error::{AppError, Result};
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;
use tracing::debug;

pub struct ScreenshotService {
    screenshot_dir: PathBuf,
}

impl ScreenshotService {
    pub fn new(config: &Config) -> Self {
        Self {
            screenshot_dir: config.screenshot_dir.clone(),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        if !self.screenshot_dir.exists() {
            fs::create_dir_all(&self.screenshot_dir)
                .await
                .map_err(|e| AppError::ScreenshotError(format!(
                    "Failed to create screenshot directory: {}", e
                )))?;
        }
        Ok(())
    }

    pub async fn save_screenshot(&self, data: &[u8], url: &str) -> Result<String> {
        let filename = self.generate_filename(url);
        let filepath = self.screenshot_dir.join(&filename);

        fs::write(&filepath, data)
            .await
            .map_err(|e| AppError::ScreenshotError(format!(
                "Failed to save screenshot: {}", e
            )))?;

        debug!("Screenshot saved: {:?}", filepath);

        Ok(format!("/screenshots/{}", filename))
    }

    fn generate_filename(&self, url: &str) -> String {
        let uuid = Uuid::new_v4();
        let sanitized_url = url
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .take(50)
            .collect::<String>();

        format!("{}_{}.png", sanitized_url, uuid)
    }

    pub async fn get_screenshot(&self, filename: &str) -> Result<Vec<u8>> {
        let filepath = self.screenshot_dir.join(filename);

        fs::read(&filepath)
            .await
            .map_err(|e| AppError::ScreenshotError(format!(
                "Failed to read screenshot: {}", e
            )))
    }

    pub async fn delete_screenshot(&self, filename: &str) -> Result<()> {
        let filepath = self.screenshot_dir.join(filename);

        if filepath.exists() {
            fs::remove_file(&filepath)
                .await
                .map_err(|e| AppError::ScreenshotError(format!(
                    "Failed to delete screenshot: {}", e
                )))?;
        }

        Ok(())
    }

    pub async fn cleanup_old_screenshots(&self, max_age_secs: u64) -> Result<usize> {
        let mut deleted = 0;

        let mut entries = fs::read_dir(&self.screenshot_dir)
            .await
            .map_err(|e| AppError::ScreenshotError(format!(
                "Failed to read screenshot directory: {}", e
            )))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| AppError::ScreenshotError(e.to_string()))?
        {
            let metadata = entry.metadata().await
                .map_err(|e| AppError::ScreenshotError(e.to_string()))?;

            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = modified.elapsed() {
                    if age.as_secs() > max_age_secs {
                        if let Err(e) = fs::remove_file(entry.path()).await {
                            debug!("Failed to delete old screenshot: {}", e);
                        } else {
                            deleted += 1;
                        }
                    }
                }
            }
        }

        Ok(deleted)
    }
}
