use crate::error::Result;
use crate::models::{ExtractedContent, ImageData, LinkData};
use html2md::parse_html;
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref MULTIPLE_NEWLINES: Regex = Regex::new(r"\n{3,}").unwrap();
    static ref TRAILING_SPACES: Regex = Regex::new(r"[ \t]+\n").unwrap();
    static ref MULTIPLE_SPACES: Regex = Regex::new(r" {2,}").unwrap();
    static ref EMPTY_LINKS: Regex = Regex::new(r"\[]\([^)]*\)").unwrap();
    static ref BROKEN_LINKS: Regex = Regex::new(r"\[([^\]]*)\]\s+\(([^)]*)\)").unwrap();
    static ref EMPTY_HEADERS: Regex = Regex::new(r"^#{1,6}\s*$").unwrap();
    static ref SVG_CONTENT: Regex = Regex::new(r"<svg[^>]*>[\s\S]*?</svg>").unwrap();
}

pub struct MarkdownService;

impl MarkdownService {
    pub fn new() -> Self {
        Self
    }

    pub fn convert_to_markdown(&self, content: &ExtractedContent) -> Result<String> {
        let cleaned_html = self.preprocess_html(&content.content);

        let markdown = parse_html(&cleaned_html);

        let tidied = self.tidy_markdown(&markdown);

        let with_metadata = self.add_metadata_header(&tidied, content);

        Ok(with_metadata)
    }

    pub fn convert_raw(&self, html: &str) -> Result<String> {
        let cleaned_html = self.preprocess_html(html);
        let markdown = parse_html(&cleaned_html);
        let tidied = self.tidy_markdown(&markdown);
        Ok(tidied)
    }

    fn preprocess_html(&self, html: &str) -> String {
        let mut result = html.to_string();

        result = SVG_CONTENT.replace_all(&result, "[SVG Image]").to_string();

        result = self.remove_style_attributes(&result);

        result = self.normalize_whitespace(&result);

        result
    }

    fn remove_style_attributes(&self, html: &str) -> String {
        let style_pattern = Regex::new(r#"\s+style="[^"]*""#).unwrap();
        let class_pattern = Regex::new(r#"\s+class="[^"]*""#).unwrap();

        let result = style_pattern.replace_all(html, "");
        class_pattern.replace_all(&result, "").to_string()
    }

    fn normalize_whitespace(&self, html: &str) -> String {
        let ws_pattern = Regex::new(r">\s+<").unwrap();
        ws_pattern.replace_all(html, "> <").to_string()
    }

    fn tidy_markdown(&self, markdown: &str) -> String {
        let mut result = markdown.to_string();

        result = BROKEN_LINKS.replace_all(&result, "[$1]($2)").to_string();

        result = EMPTY_LINKS.replace_all(&result, "").to_string();

        result = result
            .lines()
            .filter(|line| !EMPTY_HEADERS.is_match(line))
            .collect::<Vec<_>>()
            .join("\n");

        result = MULTIPLE_NEWLINES.replace_all(&result, "\n\n").to_string();

        result = TRAILING_SPACES.replace_all(&result, "\n").to_string();

        result = self.fix_list_formatting(&result);

        result = self.fix_code_blocks(&result);

        result.trim().to_string()
    }

    fn fix_list_formatting(&self, markdown: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut prev_was_list = false;
        let empty_string = String::new();

        for line in markdown.lines() {
            let trimmed = line.trim_start();
            let is_list_item = trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                    && trimmed.chars().nth(1).map_or(false, |c| c == '.' || c == ')');

            if is_list_item && !prev_was_list && !lines.is_empty() {
                let last = lines.last().unwrap_or(&empty_string);
                if !last.is_empty() {
                    lines.push(String::new());
                }
            }

            lines.push(line.to_string());
            prev_was_list = is_list_item;
        }

        lines.join("\n")
    }

    fn fix_code_blocks(&self, markdown: &str) -> String {
        let mut result = markdown.to_string();

        let code_block_pattern = Regex::new(r"```\s*\n").unwrap();
        result = code_block_pattern.replace_all(&result, "```\n").to_string();

        let broken_inline_code = Regex::new(r"`\s+`").unwrap();
        result = broken_inline_code.replace_all(&result, "` `").to_string();

        result
    }

    fn add_metadata_header(&self, markdown: &str, content: &ExtractedContent) -> String {
        let mut header_parts = Vec::new();

        if let Some(ref title) = content.title {
            header_parts.push(format!("Title: {}", title));
        }

        header_parts.push(format!("URL Source: {}", content.url));

        if let Some(ref published) = content.published_time {
            header_parts.push(format!("Published: {}", published));
        }

        if header_parts.is_empty() {
            return markdown.to_string();
        }

        format!("{}\n\n---\n\n{}", header_parts.join("\n"), markdown)
    }

    pub fn add_images_summary(&self, markdown: &str, images: &[ImageData]) -> String {
        if images.is_empty() {
            return markdown.to_string();
        }

        let mut summary = String::from("\n\n---\n\n## Images\n\n");

        for (i, image) in images.iter().enumerate() {
            let alt = image.alt.as_deref().unwrap_or("No description");
            summary.push_str(&format!("{}. [{}]({})\n", i + 1, alt, image.src));
        }

        format!("{}{}", markdown, summary)
    }

    pub fn add_links_summary(&self, markdown: &str, links: &[LinkData]) -> String {
        if links.is_empty() {
            return markdown.to_string();
        }

        let mut summary = String::from("\n\n---\n\n## Links\n\n");

        for (i, link) in links.iter().enumerate() {
            let text = link.text.as_deref().unwrap_or(&link.href);
            summary.push_str(&format!("{}. [{}]({})\n", i + 1, text, link.href));
        }

        format!("{}{}", markdown, summary)
    }

    #[allow(dead_code)]
    pub fn number_images(&self, markdown: &str, images: &[ImageData]) -> String {
        let mut result = markdown.to_string();

        for (i, image) in images.iter().enumerate() {
            let original_pattern = format!("![{}]({})",
                image.alt.as_deref().unwrap_or(""),
                image.src
            );
            let numbered = format!("![Image {}{}]({})",
                i + 1,
                image.alt.as_ref().map(|a| format!(": {}", a)).unwrap_or_default(),
                image.src
            );
            result = result.replace(&original_pattern, &numbered);
        }

        result
    }
}

impl Default for MarkdownService {
    fn default() -> Self {
        Self::new()
    }
}
