use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormat {
    Default,
    Markdown,
    Html,
    Text,
    Screenshot,
    Pageshot,
}

impl Default for ResponseFormat {
    fn default() -> Self {
        Self::Default
    }
}

impl ResponseFormat {
    pub fn from_header(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "markdown" => Self::Markdown,
            "html" => Self::Html,
            "text" => Self::Text,
            "screenshot" => Self::Screenshot,
            "pageshot" => Self::Pageshot,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CrawlerOptions {
    pub url: String,
    pub respond_with: ResponseFormat,
    pub wait_for_selector: Option<String>,
    pub target_selector: Option<String>,
    pub remove_selector: Option<String>,
    pub timeout: Option<u64>,
    pub cookies: Option<String>,
    pub proxy_url: Option<String>,
    pub user_agent: Option<String>,
    pub with_iframe: bool,
    pub with_shadow_dom: bool,
    pub no_cache: bool,
    pub cache_tolerance: Option<u64>,
    pub with_images_summary: bool,
    pub with_links_summary: bool,
    pub with_generated_alt: bool,
    pub keep_img_data_url: bool,
}

impl CrawlerOptions {
    pub fn new(url: String) -> Self {
        Self {
            url,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRequest {
    pub url: String,
    #[serde(default)]
    pub options: LoadRequestOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadRequestOptions {
    pub wait_for_selector: Option<String>,
    pub target_selector: Option<String>,
    pub remove_selector: Option<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchLoadRequest {
    pub urls: Vec<String>,
    #[serde(default)]
    pub options: LoadRequestOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWebUIRequest {
    pub urls: Vec<String>,
}
