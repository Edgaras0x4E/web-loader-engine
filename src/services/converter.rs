use crate::config::Config;
use crate::error::Result;
use crate::models::{
    CrawlerOptions, LoadResponse, PageSnapshot, ResponseFormat, ResponseMetadata,
};
use crate::services::{MarkdownService, ReadabilityService, ScraperService};
use std::time::Instant;
use tracing::debug;

pub struct ConverterService {
    #[allow(dead_code)]
    config: Config,
    scraper: ScraperService,
    readability: ReadabilityService,
    markdown: MarkdownService,
}

impl ConverterService {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            scraper: ScraperService::new(),
            readability: ReadabilityService::new(),
            markdown: MarkdownService::new(),
        }
    }

    pub async fn process(
        &self,
        html: &str,
        options: &CrawlerOptions,
    ) -> Result<LoadResponse> {
        let start = Instant::now();

        let snapshot = self.scraper.parse_html(html, options)?;

        let content = match options.respond_with {
            ResponseFormat::Html => {
                snapshot.html.clone()
            }
            ResponseFormat::Text => {
                let extracted = self.readability.extract_without_readability(&snapshot);
                extracted.text_content
            }
            ResponseFormat::Screenshot | ResponseFormat::Pageshot => {
                String::new()
            }
            ResponseFormat::Markdown | ResponseFormat::Default => {
                self.convert_to_markdown(&snapshot)?
            }
        };

        let processing_time_ms = start.elapsed().as_millis() as u64;

        let mut response = LoadResponse {
            url: options.url.clone(),
            title: snapshot.title.clone(),
            content,
            published_time: snapshot.published_time.clone(),
            images: None,
            links: None,
            screenshot_url: None,
            metadata: ResponseMetadata {
                processing_time_ms,
                cached: false,
            },
        };

        if options.with_images_summary {
            response.images = Some(
                snapshot.images.iter().map(|img| crate::models::ImageInfo {
                    src: img.src.clone(),
                    alt: img.alt.clone(),
                    width: img.width,
                    height: img.height,
                }).collect()
            );

            if matches!(options.respond_with, ResponseFormat::Default | ResponseFormat::Markdown) {
                response.content = self.markdown.add_images_summary(&response.content, &snapshot.images);
            }
        }

        if options.with_links_summary {
            response.links = Some(
                snapshot.links.iter().map(|link| crate::models::LinkInfo {
                    href: link.href.clone(),
                    text: link.text.clone(),
                }).collect()
            );

            if matches!(options.respond_with, ResponseFormat::Default | ResponseFormat::Markdown) {
                response.content = self.markdown.add_links_summary(&response.content, &snapshot.links);
            }
        }

        Ok(response)
    }

    fn convert_to_markdown(&self, snapshot: &PageSnapshot) -> Result<String> {
        debug!("Using rule-based conversion");

        let cleaned_html = self.readability.clean_html(&snapshot.html);
        let cleaned_snapshot = PageSnapshot {
            url: snapshot.url.clone(),
            html: cleaned_html,
            title: snapshot.title.clone(),
            published_time: snapshot.published_time.clone(),
            images: snapshot.images.clone(),
            links: snapshot.links.clone(),
            has_pdf: snapshot.has_pdf,
        };

        let extracted = self.readability.extract_content(&cleaned_snapshot)?;
        let markdown = self.markdown.convert_to_markdown(&extracted)?;

        Ok(markdown)
    }

    pub fn get_scraper(&self) -> &ScraperService {
        &self.scraper
    }

    pub fn get_markdown_service(&self) -> &MarkdownService {
        &self.markdown
    }
}
