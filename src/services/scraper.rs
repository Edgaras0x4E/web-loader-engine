use crate::error::{AppError, Result};
use crate::models::{ComplexityMetrics, CrawlerOptions, ImageData, LinkData, PageSnapshot};
use scraper::{Html, Selector};
use tracing::debug;

pub struct ScraperService;

impl ScraperService {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_html(&self, html: &str, options: &CrawlerOptions) -> Result<PageSnapshot> {
        let document = Html::parse_document(html);

        let title = self.extract_title(&document);

        let published_time = self.extract_published_time(&document);

        let content_html = if let Some(ref selector_str) = options.target_selector {
            self.extract_targeted_content(&document, selector_str)?
        } else {
            html.to_string()
        };

        let final_html = if let Some(ref selector_str) = options.remove_selector {
            self.remove_elements(&content_html, selector_str)?
        } else {
            content_html
        };

        let images = self.extract_images(&document, options.keep_img_data_url);

        let base_url = &options.url;
        let links = self.extract_links(&document, base_url);

        let has_pdf = self.detect_pdf(&document);

        Ok(PageSnapshot {
            url: options.url.clone(),
            html: final_html,
            title,
            published_time,
            images,
            links,
            has_pdf,
        })
    }

    pub fn calculate_complexity(&self, html: &str) -> ComplexityMetrics {
        let document = Html::parse_document(html);
        let mut metrics = ComplexityMetrics::default();

        if let Ok(selector) = Selector::parse("table") {
            metrics.table_count = document.select(&selector).count();
        }

        metrics.max_list_depth = self.calculate_list_depth(&document);

        if let Ok(selector) = Selector::parse("pre, code") {
            metrics.code_block_count = document.select(&selector).count();
        }

        metrics.has_math = self.detect_math(&document);

        metrics.is_non_english = self.detect_non_english(&document);

        if let Ok(selector) = Selector::parse("*") {
            metrics.total_elements = document.select(&selector).count();
        }

        debug!("Complexity metrics: {:?}", metrics);
        metrics
    }

    fn extract_title(&self, document: &Html) -> Option<String> {
        if let Ok(selector) = Selector::parse("meta[property='og:title']") {
            if let Some(element) = document.select(&selector).next() {
                if let Some(content) = element.value().attr("content") {
                    if !content.is_empty() {
                        return Some(content.to_string());
                    }
                }
            }
        }

        if let Ok(selector) = Selector::parse("title") {
            if let Some(element) = document.select(&selector).next() {
                let title: String = element.text().collect();
                if !title.is_empty() {
                    return Some(title.trim().to_string());
                }
            }
        }

        None
    }

    fn extract_published_time(&self, document: &Html) -> Option<String> {
        let selectors = [
            "meta[property='article:published_time']",
            "meta[name='publishedDate']",
            "meta[name='date']",
            "time[datetime]",
            "meta[property='og:article:published_time']",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    if let Some(content) = element.value().attr("content")
                        .or_else(|| element.value().attr("datetime"))
                    {
                        if !content.is_empty() {
                            return Some(content.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    fn extract_targeted_content(&self, document: &Html, selector_str: &str) -> Result<String> {
        let selector = Selector::parse(selector_str)
            .map_err(|_| AppError::ScrapingError(format!("Invalid selector: {}", selector_str)))?;

        let mut content = String::new();
        for element in document.select(&selector) {
            content.push_str(&element.html());
        }

        if content.is_empty() {
            Err(AppError::ScrapingError(format!(
                "No content found for selector: {}",
                selector_str
            )))
        } else {
            Ok(content)
        }
    }

    fn remove_elements(&self, html: &str, selector_str: &str) -> Result<String> {
        let document = Html::parse_document(html);
        let selector = Selector::parse(selector_str)
            .map_err(|_| AppError::ScrapingError(format!("Invalid selector: {}", selector_str)))?;

        let elements_to_remove: Vec<String> = document
            .select(&selector)
            .map(|el| el.html())
            .collect();

        let mut result = html.to_string();
        for element in elements_to_remove {
            result = result.replace(&element, "");
        }

        Ok(result)
    }

    fn extract_images(&self, document: &Html, keep_data_url: bool) -> Vec<ImageData> {
        let mut images = Vec::new();

        if let Ok(selector) = Selector::parse("img") {
            for element in document.select(&selector) {
                let src = element.value().attr("src")
                    .or_else(|| element.value().attr("data-src"))
                    .map(|s| s.to_string());

                if let Some(src) = src {
                    if src.starts_with("data:") && !keep_data_url {
                        continue;
                    }

                    let alt = element.value().attr("alt").map(|s| s.to_string());
                    let width = element.value().attr("width")
                        .and_then(|w| w.parse().ok());
                    let height = element.value().attr("height")
                        .and_then(|h| h.parse().ok());

                    let data_url = if src.starts_with("data:") && keep_data_url {
                        Some(src.clone())
                    } else {
                        None
                    };

                    images.push(ImageData {
                        src,
                        alt,
                        width,
                        height,
                        data_url,
                    });
                }
            }
        }

        images
    }

    fn extract_links(&self, document: &Html, base_url: &str) -> Vec<LinkData> {
        let mut links = Vec::new();
        let base_domain = url::Url::parse(base_url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()));

        if let Ok(selector) = Selector::parse("a[href]") {
            for element in document.select(&selector) {
                if let Some(href) = element.value().attr("href") {
                    let text: String = element.text().collect();
                    let text = text.trim().to_string();

                    let is_internal = if let Some(ref domain) = base_domain {
                        href.starts_with('/') || href.contains(domain)
                    } else {
                        href.starts_with('/')
                    };

                    links.push(LinkData {
                        href: href.to_string(),
                        text: if text.is_empty() { None } else { Some(text) },
                        is_internal,
                    });
                }
            }
        }

        links
    }

    fn detect_pdf(&self, document: &Html) -> bool {
        if let Ok(selector) = Selector::parse("embed[type='application/pdf'], object[type='application/pdf'], iframe[src*='.pdf']") {
            if document.select(&selector).next().is_some() {
                return true;
            }
        }

        if let Ok(selector) = Selector::parse("a[href$='.pdf']") {
            if document.select(&selector).next().is_some() {
                return true;
            }
        }

        false
    }

    fn calculate_list_depth(&self, document: &Html) -> usize {
        let mut max_depth = 0;

        if let Ok(selector) = Selector::parse("ul, ol") {
            for element in document.select(&selector) {
                let depth = self.count_list_depth(element.html().as_str(), 0);
                if depth > max_depth {
                    max_depth = depth;
                }
            }
        }

        max_depth
    }

    fn count_list_depth(&self, html: &str, current_depth: usize) -> usize {
        let doc = Html::parse_fragment(html);
        let mut max_depth = current_depth;

        if let Ok(selector) = Selector::parse("ul, ol") {
            for element in doc.select(&selector) {
                let inner_html = element.inner_html();
                let depth = self.count_list_depth(&inner_html, current_depth + 1);
                if depth > max_depth {
                    max_depth = depth;
                }
            }
        }

        max_depth
    }

    fn detect_math(&self, document: &Html) -> bool {
        if let Ok(selector) = Selector::parse("math, .MathJax, .katex, [class*='math'], [class*='latex']") {
            if document.select(&selector).next().is_some() {
                return true;
            }
        }

        let html = document.html();
        if html.contains("\\(") || html.contains("\\[") || html.contains("$$") {
            return true;
        }

        false
    }

    fn detect_non_english(&self, document: &Html) -> bool {
        if let Ok(selector) = Selector::parse("html[lang]") {
            if let Some(element) = document.select(&selector).next() {
                if let Some(lang) = element.value().attr("lang") {
                    let lang_lower = lang.to_lowercase();
                    if !lang_lower.starts_with("en") {
                        return true;
                    }
                }
            }
        }

        let text: String = document.root_element().text().collect();
        let cjk_count = text.chars().filter(|c| {
            let code = *c as u32;
            (0x4E00..=0x9FFF).contains(&code) ||
            (0x3040..=0x309F).contains(&code) ||
            (0x30A0..=0x30FF).contains(&code) ||
            (0xAC00..=0xD7AF).contains(&code)
        }).count();

        let total_chars = text.chars().count();
        if total_chars > 0 && (cjk_count as f32 / total_chars as f32) > 0.1 {
            return true;
        }

        false
    }
}

impl Default for ScraperService {
    fn default() -> Self {
        Self::new()
    }
}
