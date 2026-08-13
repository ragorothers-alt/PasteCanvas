//! Color detection and parsing utilities.
//! Per GOAL.md P1: "顏色卡片" — detect color values in captured text.

/// Represents a parsed color value.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedColor {
    pub hex: String,      // Normalized #RRGGBB
    pub original: String, // Original matched text
    pub format: ColorFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorFormat {
    Hex3,  // #RGB
    Hex6,  // #RRGGBB
    Hex8,  // #RRGGBBAA
    Rgb,   // rgb(r, g, b)
    Rgba,  // rgba(r, g, b, a)
    Hsl,   // hsl(h, s%, l%)
    Named, // CSS named color
}

/// Attempts to parse a color value from text.
/// Returns None if the text is not a recognizable color.
pub fn parse_color(text: &str) -> Option<ParsedColor> {
    let trimmed = text.trim();

    // Try hex formats
    if let Some(color) = parse_hex(trimmed) {
        return Some(color);
    }

    // Try rgb/rgba
    if let Some(color) = parse_rgb(trimmed) {
        return Some(color);
    }

    // Try hsl
    if let Some(color) = parse_hsl(trimmed) {
        return Some(color);
    }

    // Try named colors (common subset)
    if let Some(color) = parse_named(trimmed) {
        return Some(color);
    }

    None
}

fn parse_hex(text: &str) -> Option<ParsedColor> {
    if !text.starts_with('#') {
        return None;
    }

    let hex = &text[1..];
    match hex.len() {
        3 => {
            // #RGB → #RRGGBB
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let expanded: String = hex.chars().flat_map(|c| [c, c]).collect();
            Some(ParsedColor {
                hex: format!("#{}", expanded.to_uppercase()),
                original: text.to_string(),
                format: ColorFormat::Hex3,
            })
        }
        6 => {
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            Some(ParsedColor {
                hex: format!("#{}", hex.to_uppercase()),
                original: text.to_string(),
                format: ColorFormat::Hex6,
            })
        }
        8 => {
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            // Take first 6 chars for display
            Some(ParsedColor {
                hex: format!("#{}", hex[..6].to_uppercase()),
                original: text.to_string(),
                format: ColorFormat::Hex8,
            })
        }
        _ => None,
    }
}

fn parse_rgb(text: &str) -> Option<ParsedColor> {
    let lower = text.to_lowercase();
    let is_rgba = lower.starts_with("rgba(");
    let is_rgb = lower.starts_with("rgb(");

    if !is_rgb && !is_rgba {
        return None;
    }

    // Extract numbers
    let inner = text
        .trim_start_matches("rgba(")
        .trim_start_matches("rgb(")
        .trim_end_matches(')');

    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }

    let r: u8 = parts[0].parse().ok()?;
    let g: u8 = parts[1].parse().ok()?;
    let b: u8 = parts[2].parse().ok()?;

    Some(ParsedColor {
        hex: format!("#{:02X}{:02X}{:02X}", r, g, b),
        original: text.to_string(),
        format: if is_rgba {
            ColorFormat::Rgba
        } else {
            ColorFormat::Rgb
        },
    })
}

fn parse_hsl(text: &str) -> Option<ParsedColor> {
    let lower = text.to_lowercase();
    if !lower.starts_with("hsl(") {
        return None;
    }

    let inner = text
        .trim_start_matches("hsl(")
        .trim_start_matches("HSL(")
        .trim_end_matches(')');

    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }

    let h: f64 = parts[0].parse().ok()?;
    let s: f64 = parts[1].trim_end_matches('%').parse().ok()?;
    let l: f64 = parts[2].trim_end_matches('%').parse().ok()?;

    // HSL to RGB conversion
    let (r, g, b) = hsl_to_rgb(h, s / 100.0, l / 100.0);

    Some(ParsedColor {
        hex: format!("#{:02X}{:02X}{:02X}", r, g, b),
        original: text.to_string(),
        format: ColorFormat::Hsl,
    })
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r1, g1, b1) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

fn parse_named(text: &str) -> Option<ParsedColor> {
    let lower = text.to_lowercase();
    let hex = match lower.as_str() {
        "red" => "#FF0000",
        "green" => "#008000",
        "blue" => "#0000FF",
        "white" => "#FFFFFF",
        "black" => "#000000",
        "yellow" => "#FFFF00",
        "cyan" => "#00FFFF",
        "magenta" => "#FF00FF",
        "orange" => "#FFA500",
        "purple" => "#800080",
        "pink" => "#FFC0CB",
        "gray" | "grey" => "#808080",
        "brown" => "#A52A2A",
        "navy" => "#000080",
        "teal" => "#008080",
        "olive" => "#808000",
        "maroon" => "#800000",
        "silver" => "#C0C0C0",
        "gold" => "#FFD700",
        "coral" => "#FF7F50",
        "salmon" => "#FA8072",
        "indigo" => "#4B0082",
        "violet" => "#EE82EE",
        "crimson" => "#DC143C",
        "tomato" => "#FF6347",
        "turquoise" => "#40E0D0",
        "lavender" => "#E6E6FA",
        "beige" => "#F5F5DC",
        "ivory" => "#FFFFF0",
        _ => return None,
    };

    Some(ParsedColor {
        hex: hex.to_string(),
        original: text.to_string(),
        format: ColorFormat::Named,
    })
}

/// Checks if text is likely a color value (for content type classification).
pub fn is_color_text(text: &str) -> bool {
    let trimmed = text.trim();
    // Only classify as color if the entire text is a color value
    // (not just contains a color somewhere)
    trimmed.len() <= 30 && parse_color(trimmed).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex6() {
        let color = parse_color("#FF5733").unwrap();
        assert_eq!(color.hex, "#FF5733");
        assert_eq!(color.format, ColorFormat::Hex6);
    }

    #[test]
    fn parse_hex3_expands() {
        let color = parse_color("#F00").unwrap();
        assert_eq!(color.hex, "#FF0000");
        assert_eq!(color.format, ColorFormat::Hex3);
    }

    #[test]
    fn parse_hex_lowercase() {
        let color = parse_color("#ff5733").unwrap();
        assert_eq!(color.hex, "#FF5733");
    }

    #[test]
    fn parse_hex8() {
        let color = parse_color("#FF573380").unwrap();
        assert_eq!(color.hex, "#FF5733");
        assert_eq!(color.format, ColorFormat::Hex8);
    }

    #[test]
    fn parse_rgb() {
        let color = parse_color("rgb(255, 87, 51)").unwrap();
        assert_eq!(color.hex, "#FF5733");
        assert_eq!(color.format, ColorFormat::Rgb);
    }

    #[test]
    fn parse_rgba() {
        let color = parse_color("rgba(255, 87, 51, 0.5)").unwrap();
        assert_eq!(color.hex, "#FF5733");
        assert_eq!(color.format, ColorFormat::Rgba);
    }

    #[test]
    fn parse_hsl() {
        let color = parse_color("hsl(0, 100%, 50%)").unwrap();
        assert_eq!(color.hex, "#FF0000"); // Pure red
        assert_eq!(color.format, ColorFormat::Hsl);
    }

    #[test]
    fn parse_named_red() {
        let color = parse_color("red").unwrap();
        assert_eq!(color.hex, "#FF0000");
        assert_eq!(color.format, ColorFormat::Named);
    }

    #[test]
    fn parse_named_case_insensitive() {
        let color = parse_color("Turquoise").unwrap();
        assert_eq!(color.hex, "#40E0D0");
    }

    #[test]
    fn reject_invalid() {
        assert!(parse_color("not a color").is_none());
        assert!(parse_color("#GGGGGG").is_none());
        assert!(parse_color("#12345").is_none());
        assert!(parse_color("").is_none());
    }

    #[test]
    fn is_color_text_checks() {
        assert!(is_color_text("#FF5733"));
        assert!(is_color_text("red"));
        assert!(is_color_text("rgb(255, 0, 0)"));
        assert!(!is_color_text("The color is #FF5733 in this sentence"));
        assert!(!is_color_text("hello world"));
    }
}
