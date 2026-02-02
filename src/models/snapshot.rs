use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub html: String,
    pub title: Option<String>,
    pub published_time: Option<String>,
    pub images: Vec<ImageData>,
    pub links: Vec<LinkData>,
    pub has_pdf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub src: String,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkData {
    pub href: String,
    pub text: Option<String>,
    pub is_internal: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub text_content: String,
    pub published_time: Option<String>,
    pub images: Vec<ImageData>,
    pub links: Vec<LinkData>,
}

#[derive(Debug, Clone, Default)]
pub struct ComplexityMetrics {
    pub table_count: usize,
    pub max_list_depth: usize,
    pub code_block_count: usize,
    pub has_math: bool,
    pub is_non_english: bool,
    pub total_elements: usize,
}

impl ComplexityMetrics {
    pub fn calculate_score(&self) -> f32 {
        let mut score: f32 = 0.0;

        if self.table_count > 2 {
            score += 0.3;
        } else if self.table_count > 0 {
            score += 0.15;
        }

        if self.max_list_depth > 3 {
            score += 0.2;
        } else if self.max_list_depth > 1 {
            score += 0.1;
        }

        if self.code_block_count > 5 {
            score += 0.15;
        } else if self.code_block_count > 2 {
            score += 0.08;
        }

        if self.has_math {
            score += 0.25;
        }

        if self.is_non_english {
            score += 0.1;
        }

        score.min(1.0)
    }
}
