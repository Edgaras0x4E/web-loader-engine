use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadResponse {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<LinkInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_url: Option<String>,
    pub metadata: ResponseMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInfo {
    pub href: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub processing_time_ms: u64,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchLoadResponse {
    pub results: Vec<BatchLoadResult>,
    pub total_processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchLoadResult {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<LoadResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub browser_pool: BrowserPoolStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserPoolStatus {
    pub available: usize,
    pub total: usize,
    pub healthy: bool,
    pub recreation_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWebUIDocument {
    pub page_content: String,
    pub metadata: OpenWebUIMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWebUIMetadata {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}
