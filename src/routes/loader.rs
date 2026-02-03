use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use futures::future::join_all;
use std::time::Instant;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::{
    BatchLoadRequest, BatchLoadResponse, BatchLoadResult, CrawlerOptions,
    LoadRequest, LoadResponse, OpenWebUIDocument, OpenWebUIMetadata,
    OpenWebUIRequest, ResponseFormat, ResponseMetadata,
};
use crate::services::{BrowserPool, SecurityService};
use crate::AppState;

const MAX_REQUEST_RETRIES: u32 = 2;

#[axum::debug_handler]
pub async fn load_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoadRequest>,
) -> Result<Json<LoadResponse>, AppError> {
    let start = Instant::now();
    info!("Processing load request for URL: {}", request.url);

    let options = parse_options(&headers, &request.url, &request.options)?;

    let url = state.security.validate_url(&options.url)?;
    let domain = SecurityService::extract_domain(&url);

    state.security.check_circuit_breaker(&domain)?;

    state.security.check_rate_limit(&domain)?;

    if !options.no_cache {
        let cache_key = format!("{}:{:?}", options.url, options.respond_with);
        if let Some(cached) = state.cache.get_with_tolerance(&cache_key, options.cache_tolerance) {
            info!("Returning cached response for {}", options.url);
            return Ok(Json(cached));
        }
    }

    let response = process_url_with_retry(&state, &options).await?;

    state.security.record_success(&domain);

    if !options.no_cache {
        let cache_key = format!("{}:{:?}", options.url, options.respond_with);
        state.cache.set(cache_key, response.clone(), options.cache_tolerance);
    }

    info!(
        "Processed {} in {}ms",
        options.url,
        start.elapsed().as_millis(),
    );

    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn batch_load_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BatchLoadRequest>,
) -> Result<Json<BatchLoadResponse>, AppError> {
    let start = Instant::now();
    info!("Processing batch load request for {} URLs", request.urls.len());

    let domains: Vec<String> = request.urls.iter()
        .filter_map(|u| url::Url::parse(u).ok())
        .map(|u| u.host_str().unwrap_or("").to_string())
        .collect();
    state.security.check_domain_count(&domains)?;

    let futures: Vec<_> = request.urls.iter().map(|url| {
        let state = state.clone();
        let headers = headers.clone();
        let options = request.options.clone();
        let url = url.clone();

        async move {
            let load_request = LoadRequest {
                url: url.clone(),
                options,
            };

            match parse_options(&headers, &url, &load_request.options) {
                Ok(opts) => {
                    match process_url_with_retry(&state, &opts).await {
                        Ok(response) => BatchLoadResult {
                            url,
                            response: Some(response),
                            error: None,
                        },
                        Err(e) => BatchLoadResult {
                            url,
                            response: None,
                            error: Some(e.to_string()),
                        },
                    }
                }
                Err(e) => BatchLoadResult {
                    url,
                    response: None,
                    error: Some(e.to_string()),
                },
            }
        }
    }).collect();

    let results = join_all(futures).await;

    let total_time = start.elapsed().as_millis() as u64;
    info!("Batch processed {} URLs in {}ms", request.urls.len(), total_time);

    Ok(Json(BatchLoadResponse {
        results,
        total_processing_time_ms: total_time,
    }))
}

async fn process_url_with_retry(
    state: &AppState,
    options: &CrawlerOptions,
) -> Result<LoadResponse, AppError> {
    let mut last_error = None;

    for attempt in 0..=MAX_REQUEST_RETRIES {
        if attempt > 0 {
            warn!(
                "Retrying request for {} (attempt {}/{})",
                options.url,
                attempt + 1,
                MAX_REQUEST_RETRIES + 1
            );
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        match process_url(state, options).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                if BrowserPool::is_connection_error(&e) {
                    warn!(
                        "Connection error processing {}: {}, will retry",
                        options.url, e
                    );
                    state.browser_pool.invalidate_browser().await;
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::BrowserError("Failed to process URL after max retries".to_string())
    }))
}

async fn process_url(
    state: &AppState,
    options: &CrawlerOptions,
) -> Result<LoadResponse, AppError> {
    if matches!(options.respond_with, ResponseFormat::Screenshot | ResponseFormat::Pageshot) {
        return process_screenshot(state, options).await;
    }

    let page = state.browser_pool.get_page(options).await?;

    let html = state.browser_pool
        .navigate_and_wait(&page, &options.url, options)
        .await?;

    drop(page);

    let response = state.converter.process(&html, options).await?;

    Ok(response)
}

async fn process_screenshot(
    state: &AppState,
    options: &CrawlerOptions,
) -> Result<LoadResponse, AppError> {
    let full_page = matches!(options.respond_with, ResponseFormat::Pageshot);

    let page = state.browser_pool.get_page(options).await?;

    state.browser_pool
        .navigate_and_wait(&page, &options.url, options)
        .await?;

    let screenshot_data = state.browser_pool
        .take_screenshot(&page, full_page)
        .await?;

    let screenshot_url = state.screenshot_service
        .save_screenshot(&screenshot_data, &options.url)
        .await?;

    drop(page);

    Ok(LoadResponse {
        url: options.url.clone(),
        title: None,
        content: String::new(),
        published_time: None,
        images: None,
        links: None,
        screenshot_url: Some(screenshot_url),
        metadata: ResponseMetadata {
            processing_time_ms: 0,
            cached: false,
        },
    })
}

#[axum::debug_handler]
pub async fn openwebui_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OpenWebUIRequest>,
) -> Result<Json<Vec<OpenWebUIDocument>>, AppError> {
    let start = Instant::now();
    info!("Processing OpenWebUI request for {} URLs", request.urls.len());

    let domains: Vec<String> = request.urls.iter()
        .filter_map(|u| url::Url::parse(u).ok())
        .map(|u| u.host_str().unwrap_or("").to_string())
        .collect();
    state.security.check_domain_count(&domains)?;

    let futures: Vec<_> = request.urls.iter().map(|url| {
        let state = state.clone();
        let headers = headers.clone();
        let url = url.clone();

        async move {
            let load_request = LoadRequest {
                url: url.clone(),
                options: Default::default(),
            };

            match parse_options(&headers, &url, &load_request.options) {
                Ok(opts) => {
                    match process_url_with_retry(&state, &opts).await {
                        Ok(response) => Some(OpenWebUIDocument {
                            page_content: response.content,
                            metadata: OpenWebUIMetadata {
                                source: url,
                                title: response.title,
                            },
                        }),
                        Err(e) => {
                            info!("Failed to load {}: {}", url, e);
                            None
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to parse options for {}: {}", url, e);
                    None
                }
            }
        }
    }).collect();

    let results: Vec<OpenWebUIDocument> = join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    info!(
        "OpenWebUI batch processed {} URLs ({} successful) in {}ms",
        request.urls.len(),
        results.len(),
        start.elapsed().as_millis()
    );

    Ok(Json(results))
}

fn parse_options(
    headers: &HeaderMap,
    url: &str,
    request_options: &crate::models::LoadRequestOptions,
) -> Result<CrawlerOptions, AppError> {
    let get_header = |name: &str| -> Option<String> {
        headers.get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    };

    let get_bool_header = |name: &str| -> bool {
        headers.get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false)
    };

    let respond_with = get_header("x-respond-with")
        .map(|v| ResponseFormat::from_header(&v))
        .unwrap_or_default();

    Ok(CrawlerOptions {
        url: url.to_string(),
        respond_with,
        wait_for_selector: get_header("x-wait-for-selector")
            .or_else(|| request_options.wait_for_selector.clone()),
        target_selector: get_header("x-target-selector")
            .or_else(|| request_options.target_selector.clone()),
        remove_selector: get_header("x-remove-selector")
            .or_else(|| request_options.remove_selector.clone()),
        timeout: get_header("x-timeout")
            .and_then(|v| v.parse().ok())
            .or(request_options.timeout),
        cookies: get_header("x-set-cookie"),
        proxy_url: get_header("x-proxy-url"),
        user_agent: get_header("x-user-agent"),
        with_iframe: get_bool_header("x-with-iframe"),
        with_shadow_dom: get_bool_header("x-with-shadow-dom"),
        no_cache: get_bool_header("x-no-cache"),
        cache_tolerance: get_header("x-cache-tolerance").and_then(|v| v.parse().ok()),
        with_images_summary: get_bool_header("x-with-images-summary"),
        with_links_summary: get_bool_header("x-with-links-summary"),
        with_generated_alt: get_bool_header("x-with-generated-alt"),
        keep_img_data_url: get_bool_header("x-keep-img-data-url"),
    })
}
