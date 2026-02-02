use crate::error::{AppError, Result};
use crate::models::{ExtractedContent, PageSnapshot};
use readability::extractor;
use scraper::{Html, Selector};
use std::io::Cursor;
use tracing::debug;
use url::Url;

pub struct ReadabilityService;

impl ReadabilityService {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_content(&self, snapshot: &PageSnapshot) -> Result<ExtractedContent> {
        let url = Url::parse(&snapshot.url)
            .map_err(|e| AppError::ExtractionError(format!("Invalid URL: {}", e)))?;

        let mut cursor = Cursor::new(snapshot.html.as_bytes());

        match extractor::extract(&mut cursor, &url) {
            Ok(product) => {
                debug!("Readability extraction successful");

                let text_content = self.extract_text(&product.content);

                Ok(ExtractedContent {
                    url: snapshot.url.clone(),
                    title: if product.title.is_empty() {
                        snapshot.title.clone()
                    } else {
                        Some(product.title)
                    },
                    content: product.content,
                    text_content,
                    published_time: snapshot.published_time.clone(),
                    images: snapshot.images.clone(),
                    links: snapshot.links.clone(),
                })
            }
            Err(e) => {
                debug!("Readability extraction failed: {}, using raw HTML", e);
                let text_content = self.extract_text(&snapshot.html);

                Ok(ExtractedContent {
                    url: snapshot.url.clone(),
                    title: snapshot.title.clone(),
                    content: snapshot.html.clone(),
                    text_content,
                    published_time: snapshot.published_time.clone(),
                    images: snapshot.images.clone(),
                    links: snapshot.links.clone(),
                })
            }
        }
    }

    pub fn extract_without_readability(&self, snapshot: &PageSnapshot) -> ExtractedContent {
        let text_content = self.extract_text(&snapshot.html);

        ExtractedContent {
            url: snapshot.url.clone(),
            title: snapshot.title.clone(),
            content: snapshot.html.clone(),
            text_content,
            published_time: snapshot.published_time.clone(),
            images: snapshot.images.clone(),
            links: snapshot.links.clone(),
        }
    }

    fn extract_text(&self, html: &str) -> String {
        let document = Html::parse_document(html);

        let text: String = document
            .root_element()
            .text()
            .collect::<Vec<_>>()
            .join(" ");

        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn clean_html(&self, html: &str) -> String {
        use regex::Regex;

        let body_html = self.extract_body(html);

        let script_re = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let cleaned = script_re.replace_all(&body_html, "").to_string();

        let style_re = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let cleaned = style_re.replace_all(&cleaned, "").to_string();

        let noscript_re = Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").unwrap();
        let cleaned = noscript_re.replace_all(&cleaned, "").to_string();

        let svg_re = Regex::new(r"(?is)<svg[^>]*>.*?</svg>").unwrap();
        let cleaned = svg_re.replace_all(&cleaned, "").to_string();

        let canvas_re = Regex::new(r"(?is)<canvas[^>]*>.*?</canvas>").unwrap();
        let cleaned = canvas_re.replace_all(&cleaned, "").to_string();

        let comment_re = Regex::new(r"(?is)<!--.*?-->").unwrap();
        let cleaned = comment_re.replace_all(&cleaned, "").to_string();

        let data_attr_re = Regex::new(r#"\s+data-[a-z0-9-]+="[^"]*""#).unwrap();
        let cleaned = data_attr_re.replace_all(&cleaned, "").to_string();

        let event_re = Regex::new(r#"\s+on[a-z]+="[^"]*""#).unwrap();
        let cleaned = event_re.replace_all(&cleaned, "").to_string();

        let document = Html::parse_document(&cleaned);
        let mut result = cleaned.clone();

        let selectors_to_remove = [
            "nav",
            "footer",
            "header:not(article header)",
            ".advertisement", ".ad", ".ads", ".advert",
            ".social-share", ".share-buttons",
            ".comments", "#comments", ".comment-section",
            ".sidebar", "#sidebar", "aside",
            ".related-posts", ".related-articles",
            "[aria-hidden='true']",
            ".cookie-banner", ".cookie-notice",
            ".newsletter-signup", ".subscribe",
            ".popup", ".modal",
        ];

        for selector_str in &selectors_to_remove {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in document.select(&selector) {
                    result = result.replace(&element.html(), "");
                }
            }
        }

        let whitespace_re = Regex::new(r"\s+").unwrap();
        let cleaned = whitespace_re.replace_all(&result, " ").to_string();

        cleaned.trim().to_string()
    }

    fn extract_body(&self, html: &str) -> String {
        use regex::Regex;

        let body_re = Regex::new(r"(?is)<body[^>]*>(.*)</body>").unwrap();
        if let Some(captures) = body_re.captures(html) {
            if let Some(body_content) = captures.get(1) {
                return body_content.as_str().to_string();
            }
        }

        let head_re = Regex::new(r"(?is)^.*?<head[^>]*>.*?</head>").unwrap();
        let cleaned = head_re.replace(html, "").to_string();

        let doctype_re = Regex::new(r"(?is)<!DOCTYPE[^>]*>").unwrap();
        let cleaned = doctype_re.replace(&cleaned, "").to_string();

        let html_open_re = Regex::new(r"(?is)<html[^>]*>").unwrap();
        let cleaned = html_open_re.replace(&cleaned, "").to_string();

        let html_close_re = Regex::new(r"(?is)</html>").unwrap();
        let cleaned = html_close_re.replace(&cleaned, "").to_string();

        cleaned.trim().to_string()
    }
}

impl Default for ReadabilityService {
    fn default() -> Self {
        Self::new()
    }
}
