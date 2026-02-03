# Web Loader Engine

High-performance web content extraction engine built in Rust. Primary purpose is serving as an **external web loader for [OpenWebUI](https://github.com/open-webui/open-webui)**, but it's flexible enough for any use case that needs clean content extraction from web pages - RAG pipelines, content indexing, web scraping, archiving, and more.

## Features

- **OpenWebUI Compatible** - Native API support, drop-in replacement
- **Multiple Output Formats** - Markdown, HTML, plain text, screenshots
- **Readability Extraction** - Mozilla Readability algorithm for clean article content
- **JavaScript Rendering** - Chromium-based rendering for JS-heavy sites
- **Smart Caching** - Built-in response caching with configurable TTL
- **Rate Limiting** - Per-domain rate limiting and circuit breakers
- **Batch Processing** - Process multiple URLs concurrently
- **Security** - SSRF protection, blocked internal IPs, optional API key auth

## Quick Start

### Docker (Recommended)

```bash
docker build -t web-loader-engine .
docker run -d -p 14786:14786 --name web-loader web-loader-engine
```

### Docker Compose

```yaml
services:
  web-loader:
    build: .
    ports:
      - "14786:14786"
    environment:
      - BROWSER_POOL_SIZE=10
      - CACHE_TTL=3600
      # - API_KEY=your-secret-key
    volumes:
      - screenshots:/app/screenshots
    restart: unless-stopped

volumes:
  screenshots:
```

```bash
docker-compose up -d
```

### Docker Hub (Pre-built Image)

```yaml
services:
  web-loader:
    image: edgaras0x4e/web-loader-engine:latest
    ports:
      - "14786:14786"
    environment:
      - BROWSER_POOL_SIZE=10
      - CACHE_TTL=3600
      # - API_KEY=your-secret-key
    volumes:
      - screenshots:/app/screenshots
    restart: unless-stopped

volumes:
  screenshots:
```

```bash
docker-compose up -d
```

Then set OpenWebUI's web loader URL to `http://web-loader:14786`

### From Source

Requires Rust 1.70+ and Chrome/Chromium installed.

```bash
cp .env.example .env  # Configure settings
cargo build --release
./target/release/web-loader-engine
```

## Configuration

Copy the example environment file and adjust as needed:

```bash
cp .env.example .env
```

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `API_PORT` | `14786` | Server port |
| `API_KEY` | - | Optional API key for authentication |
| `CHROME_PATH` | `/usr/bin/chromium` | Path to Chrome/Chromium binary |
| `BROWSER_POOL_SIZE` | `10` | Concurrent browser pages |
| `REQUEST_TIMEOUT` | `30` | Default timeout in seconds |
| `CACHE_TTL` | `3600` | Cache lifetime in seconds |
| `SCREENSHOT_DIR` | `/app/screenshots` | Screenshot storage path |

## API

### OpenWebUI Endpoint

```bash
POST /
```

```json
{"urls": ["https://example.com/article"]}
```

Returns:

```json
[
  {
    "page_content": "# Article Title\n\nContent...",
    "metadata": {
      "source": "https://example.com/article",
      "title": "Article Title"
    }
  }
]
```

### Single URL

```bash
POST /load
```

```json
{"url": "https://example.com"}
```

Response:

```json
{
  "url": "https://example.com",
  "title": "Example Domain",
  "content": "# Example Domain\n\nThis domain is for examples...",
  "metadata": {
    "processing_time_ms": 1234,
    "cached": false
  }
}
```

### Batch

```bash
POST /load/batch
```

```json
{"urls": ["https://example.com/1", "https://example.com/2"]}
```

Response:

```json
{
  "results": [
    {
      "url": "https://example.com/1",
      "response": {
        "url": "https://example.com/1",
        "title": "Page Title",
        "content": "...",
        "metadata": {"processing_time_ms": 500, "cached": false}
      }
    }
  ],
  "total_processing_time_ms": 1234
}
```

### Health Check

```bash
GET /health
```

## Request Headers

| Header | Values | Description |
|--------|--------|-------------|
| `x-respond-with` | `markdown`, `html`, `text`, `screenshot`, `pageshot` | Output format |
| `x-wait-for-selector` | CSS selector | Wait for element before extraction |
| `x-target-selector` | CSS selector | Extract only matching content |
| `x-remove-selector` | CSS selector | Remove elements before extraction |
| `x-timeout` | seconds | Request timeout |
| `x-set-cookie` | `name=value` | Set cookies |
| `x-no-cache` | `true` | Bypass cache |
| `x-with-images-summary` | `true` | Include images list |
| `x-with-links-summary` | `true` | Include links list |
| `Authorization` | `Bearer <key>` | API key (if configured) |

### Request Body Options (all optional)

```json
{
  "url": "https://example.com",
  "options": {
    "wait_for_selector": "#content",
    "target_selector": "article",
    "remove_selector": ".ads",
    "timeout": 60
  }
}
```

## Other Use Cases

While built for OpenWebUI, this works for:

- **RAG Pipelines** - Clean content for embeddings and retrieval
- **Content Archiving** - Save readable versions of web pages
- **Web Scraping** - Extract data from JavaScript-rendered pages
- **Screenshot Services** - Programmatic page captures
- **Search Indexing** - Extract text content for indexing

## Changelog

### v0.1.1

**Browser Pool Resilience** - Fixed critical issue where dead browser connections would cause requests to hang indefinitely.

- Added automatic browser health detection with 5-second timeout on page creation
- Implemented connection error detection for `Ws(AlreadyClosed)` and related WebSocket errors
- Auto-recovery: dead browsers are now automatically recreated on connection failure
- Request-level retry logic (up to 3 retries) for transient connection errors
- Health endpoint now exposes `healthy` status and `recreation_count` for monitoring

Health response now includes:
```json
{
  "status": "ok",
  "version": "0.1.1",
  "browser_pool": {
    "available": 10,
    "total": 10,
    "healthy": true,
    "recreation_count": 1
  }
}
```

Monitor `recreation_count` increasing to track browser recovery events.

## License

MIT
