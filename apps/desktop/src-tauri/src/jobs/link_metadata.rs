//! Link metadata fetcher — SSRF-safe HTTP fetch for URL title and favicon.
//!
//! Security rules (from AGENTS.md):
//! - Only allow http and https schemes
//! - Redirects verified hop-by-hop (SEC-001), max 3
//! - Limit response size (1MB for HTML)
//! - Timeout (10 seconds)
//! - No JavaScript execution
//! - No browser cookies
//! - Reject private/loopback IPs (literal AND DNS-resolved — SEC-001)
//! - Prevent path traversal in favicon URLs

use serde::Serialize;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("invalid URL scheme: only http/https allowed")]
    InvalidScheme,

    #[error("URL resolves to private/loopback address")]
    PrivateAddress,

    #[error("DNS resolution failed")]
    DnsFailure,

    #[error("too many redirects")]
    TooManyRedirects,

    #[error("invalid redirect target")]
    InvalidRedirect,

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("response too large")]
    TooLarge,

    #[error("no title found")]
    NoTitle,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkMetadata {
    pub title: Option<String>,
    pub favicon_url: Option<String>,
}

const MAX_RESPONSE_SIZE: u64 = 1_000_000; // 1MB
const MAX_REDIRECTS: usize = 3;
const TIMEOUT_SECS: u64 = 10;

/// Fetches metadata (title, favicon) from a URL with SSRF protections.
pub async fn fetch_link_metadata(target_url: &str) -> Result<LinkMetadata, MetadataError> {
    // Validate scheme
    let parsed = url::Url::parse(target_url).map_err(|_| MetadataError::InvalidScheme)?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err(MetadataError::InvalidScheme),
    }

    // Check for private/loopback IPs in the host
    if let Some(host) = parsed.host_str() {
        if is_private_host(host) {
            return Err(MetadataError::PrivateAddress);
        }
    }

    // SEC-001: resolve DNS up front and reject ANY private record. This
    // closes rebinding attacks where a public-looking hostname resolves to
    // an internal address at connect time.
    resolve_and_check(&parsed).await?;

    // Build client WITHOUT auto-follow redirects: each hop is validated
    // individually below (SEC-001).
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("PasteCanvas/0.1 (metadata bot)")
        .build()?;

    // Follow redirects manually, validating every destination.
    let mut current_url = parsed.clone();
    let mut hops = 0usize;
    let response = loop {
        let response = client.get(current_url.clone()).send().await?;
        if let Some(addr) = response.remote_addr() {
            if is_private_ip(addr.ip()) {
                return Err(MetadataError::PrivateAddress);
            }
        }
        let status = response.status();
        if status.is_redirection() {
            hops += 1;
            if hops > MAX_REDIRECTS {
                return Err(MetadataError::TooManyRedirects);
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(MetadataError::InvalidRedirect)?;
            let next = current_url
                .join(location)
                .map_err(|_| MetadataError::InvalidRedirect)?;
            validate_fetch_target(&next).await?;
            current_url = next;
            continue;
        }
        break response;
    };

    // Check content type — only process HTML
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        return Ok(LinkMetadata {
            title: None,
            favicon_url: default_favicon_url(&current_url),
        });
    }

    // Check content length
    if let Some(len) = response.content_length() {
        if len > MAX_RESPONSE_SIZE {
            return Err(MetadataError::TooLarge);
        }
    }

    // Read body with size limit
    let bytes = response.bytes().await?;
    if bytes.len() as u64 > MAX_RESPONSE_SIZE {
        return Err(MetadataError::TooLarge);
    }

    let html = String::from_utf8_lossy(&bytes);

    // Extract title (simple regex-free parsing)
    let title = extract_title(&html);

    // Extract favicon
    let favicon_url =
        extract_favicon(&html, &current_url).or_else(|| default_favicon_url(&current_url));

    Ok(LinkMetadata { title, favicon_url })
}

/// SEC-001: validates a fetch target — scheme, hostname rules, and every
/// DNS-resolved address must be public.
async fn validate_fetch_target(url: &url::Url) -> Result<(), MetadataError> {
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(MetadataError::InvalidScheme),
    }
    if let Some(host) = url.host_str() {
        if is_private_host(host) {
            return Err(MetadataError::PrivateAddress);
        }
    }
    resolve_and_check(url).await
}

/// Resolves the URL host and rejects the target when ANY returned address is
/// private/loopback/link-local/unspecified. Blocking DNS entirely on failure
/// is intentional: metadata enrichment is optional and must never reach
/// internal networks (SEC-001).
async fn resolve_and_check(url: &url::Url) -> Result<(), MetadataError> {
    let host = match url.host_str() {
        Some(h) => h.to_string(),
        None => return Err(MetadataError::InvalidScheme),
    };
    // url::Url reports IPv6 literals bracketed ("[::1]"); strip for parsing.
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    // IP literals are classified directly; no resolution needed.
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return if is_private_ip(ip) {
            Err(MetadataError::PrivateAddress)
        } else {
            Ok(())
        };
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<SocketAddr> = match tokio::task::spawn_blocking(move || {
        (host.as_str(), port)
            .to_socket_addrs()
            .map(|iter| iter.collect::<Vec<_>>())
    })
    .await
    {
        Ok(Ok(addrs)) => addrs,
        _ => return Err(MetadataError::DnsFailure),
    };
    if addrs.is_empty() {
        return Err(MetadataError::DnsFailure);
    }
    for addr in addrs {
        if is_private_ip(addr.ip()) {
            return Err(MetadataError::PrivateAddress);
        }
    }
    Ok(())
}

/// Core classification for a resolved IP address (SEC-001).
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique-local (fc00::/7) and link-local (fe80::/10)
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Checks if a hostname is a private/loopback/link-local address.
fn is_private_host(host: &str) -> bool {
    // url::Url reports IPv6 literals bracketed ("[::1]"); strip for parsing.
    let host = host.trim_start_matches('[').trim_end_matches(']');
    // Check if it's an IP literal
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_ip(ip);
    }

    // Block common private hostnames
    let lower = host.to_lowercase();
    lower == "localhost"
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower.ends_with(".localhost")
}

/// Extracts <title> content from HTML (simple parser, no JS).
fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let after_tag = html[start..].find('>')?;
    let content_start = start + after_tag + 1;
    let end = lower[content_start..].find("</title>")?;
    let title = html[content_start..content_start + end].trim();

    if title.is_empty() {
        None
    } else {
        // Decode basic HTML entities
        let decoded = title
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'");
        Some(decoded.chars().take(500).collect())
    }
}

/// Extracts favicon URL from <link rel="icon"> tags.
fn extract_favicon(html: &str, base_url: &url::Url) -> Option<String> {
    let lower = html.to_lowercase();

    // Look for <link rel="icon" or rel="shortcut icon"
    for pattern in ["rel=\"icon\"", "rel='icon'", "rel=\"shortcut icon\""] {
        if let Some(pos) = lower.find(pattern) {
            // Find href in the same tag
            let tag_start = html[..pos].rfind('<').unwrap_or(0);
            let tag_end = html[pos..].find('>').map(|i| pos + i).unwrap_or(html.len());
            let tag = &html[tag_start..tag_end];

            if let Some(href) = extract_href(tag) {
                // Resolve relative URL
                if let Ok(resolved) = base_url.join(&href) {
                    match resolved.scheme() {
                        "http" | "https" => return Some(resolved.to_string()),
                        _ => {}
                    }
                }
            }
        }
    }

    None
}

fn extract_href(tag: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let href_pos = lower.find("href=")?;
    let after_eq = &tag[href_pos + 5..];
    let quote = after_eq.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end = after_eq[1..].find(quote)?;
    Some(after_eq[1..1 + end].to_string())
}

fn default_favicon_url(base_url: &url::Url) -> Option<String> {
    let host = base_url.host_str()?;
    Some(format!("{}://{}/favicon.ico", base_url.scheme(), host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_title_simple() {
        let html = "<html><head><title>Hello World</title></head><body></body></html>";
        assert_eq!(extract_title(html), Some("Hello World".to_string()));
    }

    #[test]
    fn extract_title_with_entities() {
        let html = "<title>Tom &amp; Jerry</title>";
        assert_eq!(extract_title(html), Some("Tom & Jerry".to_string()));
    }

    #[test]
    fn extract_title_missing() {
        let html = "<html><head></head><body></body></html>";
        assert_eq!(extract_title(html), None);
    }

    #[test]
    fn extract_title_empty() {
        let html = "<title>   </title>";
        assert_eq!(extract_title(html), None);
    }

    #[test]
    fn private_host_detection() {
        assert!(is_private_host("localhost"));
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("192.168.1.1"));
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("::1"));
        assert!(is_private_host("[::1]")); // IPv6 literal as reported by url::Url
        assert!(is_private_host("myapp.local"));
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("8.8.8.8"));
    }

    #[test]
    fn extract_favicon_from_link() {
        let html = r#"<link rel="icon" href="/static/icon.png">"#;
        let base = url::Url::parse("https://example.com/page").unwrap();
        let result = extract_favicon(html, &base);
        assert_eq!(
            result,
            Some("https://example.com/static/icon.png".to_string())
        );
    }

    #[test]
    fn default_favicon() {
        let base = url::Url::parse("https://example.com/some/path").unwrap();
        assert_eq!(
            default_favicon_url(&base),
            Some("https://example.com/favicon.ico".to_string())
        );
    }

    // --- SEC-001: resolved-address classification & target validation ---

    #[test]
    fn private_ip_classification() {
        use std::net::Ipv4Addr;
        use std::net::Ipv6Addr;
        // IPv4 blocked ranges
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 7))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 5, 5))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)))); // link-local
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255))));
        // IPv6 blocked ranges
        assert!(is_private_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_private_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        assert!(is_private_ip(IpAddr::V6(Ipv6Addr::new(
            0xfd12, 0, 0, 0, 0, 0, 0, 1
        )))); // ULA
        assert!(is_private_ip(IpAddr::V6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        )))); // link-local
        // Public addresses pass
        assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1)))); // just outside 172.16/12
        assert!(!is_private_ip(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0x4860, 0, 0, 0, 0, 0, 0x4860
        ))));
    }

    #[tokio::test]
    async fn validate_target_rejects_bad_schemes() {
        let file_url = url::Url::parse("file:///C:/secret.txt").unwrap();
        assert!(matches!(
            validate_fetch_target(&file_url).await,
            Err(MetadataError::InvalidScheme)
        ));
    }

    #[tokio::test]
    async fn validate_target_rejects_private_literals() {
        // IPv4 literal
        let v4 = url::Url::parse("http://192.168.1.10/admin").unwrap();
        assert!(matches!(
            validate_fetch_target(&v4).await,
            Err(MetadataError::PrivateAddress)
        ));
        // IPv6 literal (bracketed in URLs — the fixed path)
        let v6 = url::Url::parse("http://[::1]:8080/").unwrap();
        assert!(matches!(
            validate_fetch_target(&v6).await,
            Err(MetadataError::PrivateAddress)
        ));
        // IPv6 ULA
        let ula = url::Url::parse("http://[fd12::1]/").unwrap();
        assert!(matches!(
            validate_fetch_target(&ula).await,
            Err(MetadataError::PrivateAddress)
        ));
    }

    #[tokio::test]
    async fn validate_target_rejects_private_hostnames() {
        let local = url::Url::parse("http://localhost:3000/").unwrap();
        assert!(matches!(
            validate_fetch_target(&local).await,
            Err(MetadataError::PrivateAddress)
        ));
    }

    #[tokio::test]
    async fn public_literal_passes_without_dns() {
        // Public IP literals short-circuit — no network/DNS needed.
        let public = url::Url::parse("https://8.8.8.8/").unwrap();
        assert!(validate_fetch_target(&public).await.is_ok());
    }
}
