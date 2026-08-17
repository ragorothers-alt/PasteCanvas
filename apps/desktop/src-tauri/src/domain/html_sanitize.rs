//! HTML sanitization for clipboard CF_HTML / rich text.
//! Treats HTML as untrusted input (same rule as SVG).

/// Parses the Windows CF_HTML envelope and returns the fragment (or the HTML
/// body if fragment markers are missing).
pub fn extract_cf_html_fragment(input: &str) -> String {
    let bytes = input.as_bytes();
    let start =
        header_offset(input, "StartFragment:").or_else(|| header_offset(input, "StartHTML:"));
    let end = header_offset(input, "EndFragment:").or_else(|| header_offset(input, "EndHTML:"));
    if let (Some(s), Some(e)) = (start, end) {
        if s < e && e <= bytes.len() {
            if let Ok(slice) = std::str::from_utf8(&bytes[s..e]) {
                return slice.to_string();
            }
        }
    }
    if let Some(idx) = input.to_ascii_lowercase().find("<html") {
        return input[idx..].to_string();
    }
    input.to_string()
}

fn header_offset(input: &str, key: &str) -> Option<usize> {
    let lower = input.to_ascii_lowercase();
    let key_l = key.to_ascii_lowercase();
    let pos = lower.find(&key_l)?;
    let rest = input[pos + key.len()..].lines().next()?.trim();
    rest.parse::<usize>().ok()
}

/// Strips tags to produce a plain-text fallback.
pub fn html_to_plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut skip_depth = 0u32;
    let mut i = 0;
    let chars: Vec<char> = html.chars().collect();
    while i < chars.len() {
        if chars[i] == '<' {
            let rest: String = chars[i..]
                .iter()
                .take(16)
                .collect::<String>()
                .to_ascii_lowercase();
            if rest.starts_with("<script") || rest.starts_with("<style") {
                skip_depth += 1;
            } else if rest.starts_with("</script") || rest.starts_with("</style") {
                skip_depth = skip_depth.saturating_sub(1);
            }
            in_tag = true;
            i += 1;
            continue;
        }
        if chars[i] == '>' && in_tag {
            in_tag = false;
            i += 1;
            continue;
        }
        if !in_tag && skip_depth == 0 {
            out.push(chars[i]);
        }
        i += 1;
    }
    decode_basic_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whitelist rewriter: only safe tags/attributes survive.
pub fn sanitize_html(input: &str) -> String {
    let fragment = extract_cf_html_fragment(input);
    rewrite_whitelist(&fragment)
}

const ALLOWED_TAGS: &[&str] = &[
    "p",
    "br",
    "b",
    "i",
    "u",
    "strong",
    "em",
    "a",
    "ul",
    "ol",
    "li",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "span",
    "div",
    "blockquote",
    "pre",
    "code",
    "hr",
    "table",
    "thead",
    "tbody",
    "tr",
    "td",
    "th",
];

const ALLOWED_ATTRS: &[&str] = &["href", "title", "colspan", "rowspan"];

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
        if starts_at(&chars, i, "<!--") {
            i = skip_until(&chars, i + 4, "-->");
            continue;
        }
        match parse_tag(&chars, i) {
            None => {
                if skip_stack.is_empty() {
                    out.push('<');
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
                    } else if ALLOWED_TAGS.contains(&name_l.as_str()) {
                        out.push_str("</");
                        out.push_str(&name_l);
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
                if !ALLOWED_TAGS.contains(&name_l.as_str()) {
                    if !tag.self_closing {
                        skip_stack.push(name_l);
                    }
                    continue;
                }
                out.push('<');
                out.push_str(&name_l);
                for (aname, aval) in tag.attrs {
                    let al = aname.to_ascii_lowercase();
                    if !ALLOWED_ATTRS.contains(&al.as_str()) {
                        continue;
                    }
                    if al == "href" && !is_safe_href(&aval) {
                        continue;
                    }
                    out.push(' ');
                    out.push_str(&al);
                    out.push_str("=\"");
                    out.push_str(&escape_attr(&aval));
                    out.push('"');
                }
                if tag.self_closing || name_l == "br" || name_l == "hr" {
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
                return Some((
                    ParsedTag {
                        name,
                        closing,
                        self_closing: false,
                        attrs,
                    },
                    i + 1,
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
                    let q = chars[i];
                    i += 1;
                    let vstart = i;
                    while i < chars.len() && chars[i] != q {
                        i += 1;
                    }
                    let value: String = chars[vstart..i].iter().collect();
                    if i < chars.len() {
                        i += 1;
                    }
                    value
                } else {
                    let vstart = i;
                    while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '>' {
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
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'
}

fn starts_at(chars: &[char], i: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    i + p.len() <= chars.len() && chars[i..i + p.len()] == p[..]
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

fn is_safe_href(value: &str) -> bool {
    let t = value.trim();
    let lower = t.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with('#')
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn decode_basic_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fragment_from_cf_html() {
        let raw = "Version:0.9\r\n<html><body><!--StartFragment--><b>Hi</b><!--EndFragment--></body></html>";
        let frag = extract_cf_html_fragment(raw);
        assert!(frag.contains("<b>Hi</b>"), "frag={frag}");
    }

    #[test]
    fn extracts_by_byte_offsets() {
        let body = "<b>Hi</b>";
        let header = format!("StartFragment:{:010}\r\nEndFragment:{:010}\r\n", 0, 0);
        let start = header.len();
        let end = start + body.len();
        let header = format!("StartFragment:{:010}\r\nEndFragment:{:010}\r\n", start, end);
        assert_eq!(header.len(), start);
        let raw = format!("{header}{body}");
        assert_eq!(extract_cf_html_fragment(&raw), body);
    }

    #[test]
    fn sanitize_strips_script_and_handlers() {
        let html = r#"<p onclick="alert(1)">ok<script>evil()</script></p>"#;
        let clean = sanitize_html(html);
        assert!(clean.contains("<p>"));
        assert!(clean.contains("ok"));
        assert!(!clean.contains("script"));
        assert!(!clean.contains("onclick"));
        assert!(!clean.contains("evil"));
    }

    #[test]
    fn sanitize_keeps_safe_link_drops_js() {
        let html = r#"<a href="https://example.com">x</a><a href="javascript:alert(1)">y</a>"#;
        let clean = sanitize_html(html);
        assert!(clean.contains("https://example.com"));
        assert!(!clean.contains("javascript:"));
    }

    #[test]
    fn plain_text_strips_tags() {
        let text = html_to_plain_text("<p>Hello <b>world</b></p><script>skip</script>");
        assert_eq!(text, "Hello world");
    }
}
