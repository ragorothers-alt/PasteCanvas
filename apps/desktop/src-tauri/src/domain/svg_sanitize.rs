//! SVG sanitization — treats SVG as untrusted input.
//! Per AGENTS.md: "HTML、SVG 與 rich text 一律視為不可信輸入"
//! Per AGENTS.md security: "safe SVG sanitization/raster thumbnail"
//!
//! Whitelist rewriter: only known-safe tags and attributes are emitted.
//! URL-bearing values are entity-decoded and compacted before scheme checks
//! so `jav&#x61;script:`, unquoted handlers, protocol-relative URLs, and
//! `data:`/`file:` cannot slip through.

/// Sanitizes SVG content by rewriting it through a tag/attribute allow-list.
/// Returns the sanitized SVG string, or None if the input is not valid SVG.
pub fn sanitize_svg(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.to_ascii_lowercase().contains("<svg") {
        return None;
    }

    let rewritten = rewrite_whitelist(trimmed);
    if !rewritten.to_ascii_lowercase().contains("<svg") {
        return None;
    }
    Some(rewritten)
}

/// Checks if SVG content is safe (no dangerous elements detected).
/// Used as a defense-in-depth gate after sanitization.
pub fn is_svg_safe(input: &str) -> bool {
    let lower = input.to_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = decode_entities(&compact);

    !lower.contains("<script")
        && !lower.contains("<foreignobject")
        && !lower.contains("<iframe")
        && !lower.contains("<embed")
        && !lower.contains("<object")
        && !lower.contains("<style")
        && !decoded.contains("javascript:")
        && !decoded.contains("vbscript:")
        && !decoded.contains("data:")
        && !decoded.contains("file:")
        && !has_event_handler(&lower)
        && !lower.contains("expression(")
}

fn has_event_handler(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r' {
            if bytes[i + 1] == b'o' && bytes[i + 2] == b'n' {
                let mut j = i + 3;
                while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                    j += 1;
                }
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'=' && j > i + 3 {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

const ALLOWED_TAGS: &[&str] = &[
    "svg",
    "g",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "defs",
    "lineargradient",
    "radialgradient",
    "stop",
    "clippath",
    "mask",
    "use",
    "symbol",
    "marker",
    "title",
    "desc",
    "metadata",
    "pattern",
];

const ALLOWED_ATTRS: &[&str] = &[
    "id",
    "class",
    "viewbox",
    "width",
    "height",
    "x",
    "y",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "dx",
    "dy",
    "d",
    "points",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-opacity",
    "fill-opacity",
    "fill-rule",
    "clip-rule",
    "opacity",
    "transform",
    "gradientunits",
    "gradienttransform",
    "offset",
    "stop-color",
    "stop-opacity",
    "clip-path",
    "clippathunits",
    "mask",
    "maskunits",
    "maskcontentunits",
    "preserveaspectratio",
    "xmlns",
    "xmlns:xlink",
    "version",
    "overflow",
    "visibility",
    "display",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-anchor",
    "dominant-baseline",
    "alignment-baseline",
    "letter-spacing",
    "word-spacing",
    "rotate",
    "textlength",
    "lengthadjust",
    "spreadmethod",
    "fx",
    "fy",
    "fr",
    "patternunits",
    "patterncontentunits",
    "patterntransform",
    "marker-start",
    "marker-mid",
    "marker-end",
    "markerunits",
    "markerwidth",
    "markerheight",
    "refx",
    "refy",
    "orient",
    "xml:space",
    "role",
    "aria-label",
    "aria-hidden",
];

fn is_allowed_tag(name: &str) -> bool {
    ALLOWED_TAGS.contains(&name.to_ascii_lowercase().as_str())
}

fn is_allowed_attr(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ALLOWED_ATTRS.contains(&lower.as_str()) || lower == "href" || lower == "xlink:href"
}

fn rewrite_whitelist(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut out = String::with_capacity(input.len());
    let mut skip_stack: Vec<String> = Vec::new();

    while i < chars.len() {
        if chars[i] != '<' {
            if skip_stack.is_empty() {
                out.push(chars[i]);
            }
            i += 1;
            continue;
        }

        if starts_with_at(&chars, i, "<!--") {
            i = skip_until(&chars, i + 4, "-->");
            continue;
        }
        if starts_with_at(&chars, i, "<![CDATA[") {
            i = skip_until(&chars, i + 9, "]]>");
            continue;
        }
        if starts_with_at(&chars, i, "<?") {
            i = skip_until(&chars, i + 2, "?>");
            continue;
        }
        if starts_with_at(&chars, i, "<!") {
            i = skip_until(&chars, i + 2, ">");
            continue;
        }

        match parse_tag(&chars, i) {
            None => {
                if skip_stack.is_empty() {
                    out.push(chars[i]);
                }
                i += 1;
            }
            Some((tag, next)) => {
                i = next;
                let name_l = tag.name.to_ascii_lowercase();
                if tag.closing {
                    if let Some(last) = skip_stack.last() {
                        if last == &name_l {
                            skip_stack.pop();
                        }
                    } else if is_allowed_tag(&name_l) {
                        out.push_str("</");
                        out.push_str(&tag.name);
                        out.push('>');
                    }
                    continue;
                }

                if !skip_stack.is_empty() {
                    if !tag.self_closing {
                        skip_stack.push(name_l);
                    }
                    continue;
                }

                if !is_allowed_tag(&name_l) {
                    if !tag.self_closing {
                        skip_stack.push(name_l);
                    }
                    continue;
                }

                out.push('<');
                out.push_str(&tag.name);
                for (aname, aval) in tag.attrs {
                    if !is_allowed_attr(&aname) {
                        continue;
                    }
                    if !attr_value_allowed(&aname, &aval) {
                        continue;
                    }
                    out.push(' ');
                    out.push_str(&aname);
                    out.push('=');
                    out.push('"');
                    out.push_str(&escape_attr(&aval));
                    out.push('"');
                }
                if tag.self_closing {
                    out.push_str("/>");
                } else {
                    out.push('>');
                }
            }
        }
    }

    out
}

struct ParsedTag {
    name: String,
    closing: bool,
    self_closing: bool,
    attrs: Vec<(String, String)>,
}

fn parse_tag(chars: &[char], start: usize) -> Option<(ParsedTag, usize)> {
    if start >= chars.len() || chars[start] != '<' {
        return None;
    }
    let mut i = start + 1;
    let closing = i < chars.len() && chars[i] == '/';
    if closing {
        i += 1;
    }

    let name_start = i;
    while i < chars.len() && is_name_char(chars[i]) {
        i += 1;
    }
    if i == name_start {
        return None;
    }
    let name: String = chars[name_start..i].iter().collect();

    let mut attrs = Vec::new();
    if !closing {
        loop {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i >= chars.len() {
                return None;
            }
            if chars[i] == '>' {
                i += 1;
                return Some((
                    ParsedTag {
                        name,
                        closing,
                        self_closing: false,
                        attrs,
                    },
                    i,
                ));
            }
            if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '>' {
                return Some((
                    ParsedTag {
                        name,
                        closing,
                        self_closing: true,
                        attrs,
                    },
                    i + 2,
                ));
            }

            let aname_start = i;
            while i < chars.len() && is_name_char(chars[i]) {
                i += 1;
            }
            if i == aname_start {
                // Malformed — skip to end of tag.
                while i < chars.len() && chars[i] != '>' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
                return Some((
                    ParsedTag {
                        name,
                        closing,
                        self_closing: false,
                        attrs,
                    },
                    i,
                ));
            }
            let aname: String = chars[aname_start..i].iter().collect();
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let aval = if i < chars.len() && chars[i] == '=' {
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                    let quote = chars[i];
                    i += 1;
                    let vstart = i;
                    while i < chars.len() && chars[i] != quote {
                        i += 1;
                    }
                    let value: String = chars[vstart..i].iter().collect();
                    if i < chars.len() {
                        i += 1;
                    }
                    value
                } else {
                    let vstart = i;
                    while i < chars.len()
                        && !chars[i].is_whitespace()
                        && chars[i] != '>'
                        && chars[i] != '/'
                    {
                        i += 1;
                    }
                    chars[vstart..i].iter().collect()
                }
            } else {
                String::new()
            };
            attrs.push((aname, aval));
        }
    }

    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i < chars.len() && chars[i] == '>' {
        i += 1;
    }
    Some((
        ParsedTag {
            name,
            closing,
            self_closing: false,
            attrs,
        },
        i,
    ))
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_'
}

fn starts_with_at(chars: &[char], i: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    if i + p.len() > chars.len() {
        return false;
    }
    chars[i..i + p.len()]
        .iter()
        .zip(p.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn skip_until(chars: &[char], mut i: usize, end: &str) -> usize {
    let e: Vec<char> = end.chars().collect();
    while i + e.len() <= chars.len() {
        if chars[i..i + e.len()] == e[..] {
            return i + e.len();
        }
        i += 1;
    }
    chars.len()
}

fn attr_value_allowed(name: &str, value: &str) -> bool {
    let name_l = name.to_ascii_lowercase();
    // Event handlers are on*; keep allow-listed names like `orient`.
    if name_l.starts_with("on") && !ALLOWED_ATTRS.contains(&name_l.as_str()) {
        return false;
    }
    if name_l == "style" {
        return false;
    }
    if name_l == "href" || name_l == "xlink:href" || name_l == "src" {
        return is_safe_url(value);
    }
    if name_l == "fill" || name_l == "stroke" || name_l == "clip-path" || name_l == "mask" {
        return is_safe_paint(value);
    }
    let decoded = decode_entities(value);
    let compact: String = decoded
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    !compact.contains("javascript:")
        && !compact.contains("vbscript:")
        && !compact.contains("data:")
        && !compact.contains("file:")
}

fn is_safe_url(value: &str) -> bool {
    let decoded = decode_entities(value);
    let compact: String = decoded
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let lower = compact.to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    lower.starts_with('#') && !lower[1..].contains(':')
}

fn is_safe_paint(value: &str) -> bool {
    let decoded = decode_entities(value);
    let compact: String = decoded
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let lower = compact.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("url(") {
        let inner = rest
            .trim_end_matches(')')
            .trim_matches(|c| c == '"' || c == '\'');
        return inner.starts_with('#') && !inner.contains(':');
    }
    !lower.contains("javascript:")
        && !lower.contains("vbscript:")
        && !lower.contains("data:")
        && !lower.contains("file:")
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        let mut ent = String::new();
        let mut found = false;
        while let Some(&n) = chars.peek() {
            if n == ';' {
                chars.next();
                found = true;
                break;
            }
            if n == '&' || ent.len() > 16 {
                break;
            }
            ent.push(n);
            chars.next();
        }
        if found {
            if let Some(ch) = decode_one(&ent) {
                out.push(ch);
            } else {
                out.push('&');
                out.push_str(&ent);
                out.push(';');
            }
        } else {
            out.push('&');
            out.push_str(&ent);
        }
    }
    out
}

fn decode_one(ent: &str) -> Option<char> {
    match ent {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16)
            .ok()
            .and_then(char::from_u32),
        _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
        _ => None,
    }
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

    #[test]
    fn strips_entity_encoded_javascript() {
        let svg = r#"<svg><use href="jav&#x61;script:alert(1)"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.to_lowercase().contains("javascript"));
        assert!(!result.contains("alert"));
    }

    #[test]
    fn strips_unquoted_event_handler() {
        let svg = r#"<svg><rect onclick=alert(1) width="10" height="10"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("onclick"));
        assert!(!result.contains("alert"));
        assert!(result.contains("rect"));
    }

    #[test]
    fn strips_protocol_relative_and_data_urls() {
        let svg = r#"<svg><use href="//evil.example/x" /><use xlink:href="data:text/html,x"/><use href="file:///etc/passwd"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("evil.example"));
        assert!(!result.contains("data:"));
        assert!(!result.contains("file:"));
    }

    #[test]
    fn strips_style_element_and_attribute() {
        let svg = r#"<svg><style>@import url(https://evil)</style><rect style="fill:url(https://x)" width="1"/></svg>"#;
        let result = sanitize_svg(svg).unwrap();
        assert!(!result.contains("<style"));
        assert!(!result.contains("@import"));
        assert!(!result.contains("style="));
        assert!(result.contains("rect"));
    }

    #[test]
    fn sanitized_output_is_safe() {
        let dirty = r#"<svg onload="alert(1)"><script>x</script><style>y</style><a href="javascript:1"/><rect onclick=alert(2) fill="url(#ok)"/></svg>"#;
        let result = sanitize_svg(dirty).unwrap();
        assert!(is_svg_safe(&result));
    }
}
