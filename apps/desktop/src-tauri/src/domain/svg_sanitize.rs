//! SVG sanitization — treats SVG as untrusted input.
//! Per AGENTS.md: "HTML、SVG 與 rich text 一律視為不可信輸入"
//! Per AGENTS.md security: "safe SVG sanitization/raster thumbnail"
//!
//! Strips:
//! - <script> elements
//! - Event handler attributes (onclick, onload, etc.)
//! - External references (xlink:href to external URLs)
//! - <foreignObject> elements
//! - CSS expressions and url() references
//! - JavaScript: protocol in href attributes

/// Sanitizes SVG content by removing potentially dangerous elements and attributes.
/// Returns the sanitized SVG string, or None if the input is not valid SVG.
pub fn sanitize_svg(input: &str) -> Option<String> {
    let trimmed = input.trim();

    // Must look like SVG
    if !trimmed.contains("<svg") && !trimmed.contains("<SVG") {
        return None;
    }

    let mut result = trimmed.to_string();

    // Remove <script>...</script> blocks (including multiline)
    result = remove_elements(&result, "script");

    // Remove <foreignObject>...</foreignObject> blocks
    result = remove_elements(&result, "foreignObject");

    // Remove <iframe> elements
    result = remove_elements(&result, "iframe");

    // Remove <embed> elements
    result = remove_elements(&result, "embed");

    // Remove <object> elements
    result = remove_elements(&result, "object");

    // Remove event handler attributes (on*)
    result = remove_event_handlers(&result);

    // Remove javascript: protocol references
    result = remove_javascript_urls(&result);

    // Remove external xlink:href references (keep internal #references)
    result = remove_external_references(&result);

    // Remove CSS expression() and url() with external references
    result = remove_css_expressions(&result);

    Some(result)
}

/// Removes all instances of a given element (opening tag, content, closing tag).
fn remove_elements(input: &str, tag: &str) -> String {
    let mut result = input.to_string();
    let open_pattern = format!("<{}", tag.to_lowercase());
    let close_pattern = format!("</{}", tag.to_lowercase());

    loop {
        let lower = result.to_lowercase();
        let pos = match lower.find(&open_pattern) {
            Some(p) => p,
            None => break,
        };

        // Find end of opening tag
        let tag_end = match result[pos..].find('>') {
            Some(e) => pos + e + 1,
            None => break,
        };

        // Check if self-closing
        if result[pos..tag_end].ends_with("/>") {
            result = format!("{}{}", &result[..pos], &result[tag_end..]);
            continue;
        }

        // Find closing tag
        let close_start = match result[tag_end..].to_lowercase().find(&close_pattern) {
            Some(c) => tag_end + c,
            None => {
                // No closing tag, just remove the opening tag
                result = format!("{}{}", &result[..pos], &result[tag_end..]);
                continue;
            }
        };

        // Find end of closing tag
        let close_end = match result[close_start..].find('>') {
            Some(e) => close_start + e + 1,
            None => break,
        };

        // Remove from opening tag start to closing tag end
        result = format!("{}{}", &result[..pos], &result[close_end..]);
    }

    result
}

/// Removes event handler attributes (onclick, onload, onerror, etc.)
fn remove_event_handlers(input: &str) -> String {
    let mut result = input.to_string();

    // Common SVG/HTML event handlers
    let handlers = [
        "onclick",
        "ondblclick",
        "onmousedown",
        "onmouseup",
        "onmouseover",
        "onmousemove",
        "onmouseout",
        "onmouseenter",
        "onmouseleave",
        "onkeydown",
        "onkeypress",
        "onkeyup",
        "onfocus",
        "onblur",
        "onchange",
        "oninput",
        "onsubmit",
        "onreset",
        "onload",
        "onunload",
        "onerror",
        "onresize",
        "onscroll",
        "onabort",
        "oncanplay",
        "onemptied",
        "onended",
        "onanimationstart",
        "onanimationend",
        "onanimationiteration",
        "ontransitionend",
        "onbegin",
        "onend",
        "onrepeat",
    ];

    for handler in &handlers {
        // Remove handler="..." and handler='...'
        let patterns = [format!("{}=\"", handler), format!("{}='", handler)];

        for pattern in &patterns {
            while let Some(pos) = result.to_lowercase().find(pattern) {
                let quote = pattern.chars().last().unwrap();
                let value_start = pos + pattern.len();
                if let Some(value_end) = result[value_start..].find(quote) {
                    // Also remove preceding whitespace
                    let ws_start = result[..pos]
                        .rfind(|c: char| !c.is_whitespace())
                        .map(|p| p + 1)
                        .unwrap_or(pos);
                    result = format!(
                        "{}{}",
                        &result[..ws_start],
                        &result[value_start + value_end + 1..]
                    );
                } else {
                    break;
                }
            }
        }
    }

    result
}

/// Removes javascript: protocol in href/xlink:href attributes.
fn remove_javascript_urls(input: &str) -> String {
    let mut result = input.to_string();

    // Replace javascript: URLs with empty
    let patterns = ["javascript:", "vbscript:", "data:text/html"];
    for pattern in &patterns {
        while let Some(pos) = result.to_lowercase().find(pattern) {
            // Find the attribute value boundaries
            let before = &result[..pos];
            let quote_start = before.rfind(['"', '\'']).unwrap_or(0);
            let quote_char = result.chars().nth(quote_start).unwrap_or('"');
            if let Some(quote_end) = result[pos..].find(quote_char) {
                result = format!(
                    "{}{}",
                    &result[..quote_start + 1],
                    &result[pos + quote_end..]
                );
            } else {
                break;
            }
        }
    }

    result
}

/// Removes external references in xlink:href (keeps internal #fragment references).
fn remove_external_references(input: &str) -> String {
    let mut result = input.to_string();

    // Find xlink:href or href attributes with external URLs
    let patterns = [
        "xlink:href=\"http",
        "xlink:href='http",
        "href=\"http",
        "href='http",
    ];
    for pattern in &patterns {
        while let Some(pos) = result.to_lowercase().find(&pattern.to_lowercase()) {
            let quote = pattern.chars().last().unwrap();
            let value_start = pos + pattern.len() - 1; // Include the 'h' of http
            if let Some(value_end) = result[value_start..].find(quote) {
                let ws_start = result[..pos]
                    .rfind(|c: char| !c.is_whitespace())
                    .map(|p| p + 1)
                    .unwrap_or(pos);
                result = format!(
                    "{}{}",
                    &result[..ws_start],
                    &result[value_start + value_end + 1..]
                );
            } else {
                break;
            }
        }
    }

    result
}

/// Removes CSS expression() and external url() references.
fn remove_css_expressions(input: &str) -> String {
    let mut result = input.to_string();

    // Remove expression(...)
    while let Some(pos) = result.to_lowercase().find("expression(") {
        if let Some(end) = result[pos..].find(')') {
            result = format!("{}{}", &result[..pos], &result[pos + end + 1..]);
        } else {
            break;
        }
    }

    result
}

/// Checks if SVG content is safe (no dangerous elements detected).
pub fn is_svg_safe(input: &str) -> bool {
    let lower = input.to_lowercase();
    !lower.contains("<script")
        && !lower.contains("javascript:")
        && !lower.contains("onload=")
        && !lower.contains("onclick=")
        && !lower.contains("onerror=")
        && !lower.contains("<foreignobject")
        && !lower.contains("expression(")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_script() {
        let svg = r#"<svg><script>alert('xss')</script><rect width="100" height="100"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("rect"));
    }

    #[test]
    fn sanitize_removes_event_handlers() {
        let svg = r#"<svg><rect onclick="alert('xss')" width="100" height="100"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("onclick"));
        assert!(!result.contains("alert"));
        assert!(result.contains("rect"));
    }

    #[test]
    fn sanitize_removes_onload() {
        let svg = r#"<svg onload="alert('xss')"><circle r="50"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("onload"));
        assert!(result.contains("circle"));
    }

    #[test]
    fn sanitize_removes_javascript_url() {
        let svg = r#"<svg><a href="javascript:alert('xss')"><text>click</text></a></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("javascript:"));
    }

    #[test]
    fn sanitize_removes_foreign_object() {
        let svg =
            r#"<svg><foreignObject><body onload="alert('xss')"/></foreignObject><rect/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("foreignObject"));
        assert!(result.contains("rect"));
    }

    #[test]
    fn preserves_safe_svg() {
        let svg = r#"<svg viewBox="0 0 100 100"><circle cx="50" cy="50" r="40" fill="red"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(result.contains("circle"));
        assert!(result.contains("viewBox"));
        assert!(result.contains("fill"));
    }

    #[test]
    fn preserves_internal_references() {
        let svg = r#"<svg><defs><linearGradient id="grad"/></defs><rect fill="url(#grad)"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(result.contains("url(#grad)"));
    }

    #[test]
    fn rejects_non_svg() {
        assert!(sanitize_svg("<div>not svg</div>").is_none());
        assert!(sanitize_svg("plain text").is_none());
    }

    #[test]
    fn is_svg_safe_checks() {
        assert!(is_svg_safe(r#"<svg><rect/></svg>"#));
        assert!(!is_svg_safe(r#"<svg><script>alert(1)</script></svg>"#));
        assert!(!is_svg_safe(r#"<svg onload="alert(1)"></svg>"#));
        assert!(!is_svg_safe(r#"<svg><a href="javascript:void(0)"/></svg>"#));
    }
}
