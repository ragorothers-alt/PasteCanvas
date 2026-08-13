//! Font format detection and metadata utilities.
//! Per GOAL.md P1: "字型檔預覽" — detect font formats without installing.
//! Per AGENTS.md: "font metadata/sample without install"

/// Represents a detected font format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontFormat {
    Ttf,   // TrueType
    Otf,   // OpenType (CFF)
    Woff,  // Web Open Font Format
    Woff2, // Web Open Font Format 2
}

impl FontFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ttf => "TrueType",
            Self::Otf => "OpenType",
            Self::Woff => "WOFF",
            Self::Woff2 => "WOFF2",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Ttf => "ttf",
            Self::Otf => "otf",
            Self::Woff => "woff",
            Self::Woff2 => "woff2",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
            Self::Woff => "font/woff",
            Self::Woff2 => "font/woff2",
        }
    }
}

/// Font metadata extracted from file.
#[derive(Debug, Clone)]
pub struct FontMetadata {
    pub format: FontFormat,
    pub file_name: String,
    pub file_size: u64,
    pub family_name: Option<String>,
}

/// Detects font format from magic bytes.
pub fn detect_font_format(bytes: &[u8]) -> Option<FontFormat> {
    if bytes.len() < 4 {
        return None;
    }

    match &bytes[..4] {
        // TrueType: 00 01 00 00
        [0x00, 0x01, 0x00, 0x00] => Some(FontFormat::Ttf),
        // OpenType (CFF): OTTO
        [0x4F, 0x54, 0x54, 0x4F] => Some(FontFormat::Otf),
        // WOFF: wOFF
        [0x77, 0x4F, 0x46, 0x46] => Some(FontFormat::Woff),
        // WOFF2: wOF2
        [0x77, 0x4F, 0x46, 0x32] => Some(FontFormat::Woff2),
        _ => None,
    }
}

/// Detects font format from file extension.
pub fn detect_font_from_extension(filename: &str) -> Option<FontFormat> {
    let lower = filename.to_lowercase();
    if lower.ends_with(".ttf") {
        Some(FontFormat::Ttf)
    } else if lower.ends_with(".otf") {
        Some(FontFormat::Otf)
    } else if lower.ends_with(".woff") {
        Some(FontFormat::Woff)
    } else if lower.ends_with(".woff2") {
        Some(FontFormat::Woff2)
    } else {
        None
    }
}

/// Checks if a filename has a font extension.
pub fn is_font_filename(filename: &str) -> bool {
    detect_font_from_extension(filename).is_some()
}

/// Extracts a display-friendly family name from a font filename.
/// E.g., "Roboto-Bold.ttf" → "Roboto Bold"
pub fn family_name_from_filename(filename: &str) -> String {
    // Remove extension
    let without_ext = filename
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or(filename);

    // Split on common separators and clean up
    without_ext
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Creates FontMetadata from available information.
pub fn create_font_metadata(
    filename: &str,
    file_size: u64,
    bytes: Option<&[u8]>,
) -> Option<FontMetadata> {
    // Try magic bytes first, then extension
    let format = bytes
        .and_then(detect_font_format)
        .or_else(|| detect_font_from_extension(filename))?;

    Some(FontMetadata {
        format,
        file_name: filename.to_string(),
        file_size,
        family_name: Some(family_name_from_filename(filename)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ttf_magic() {
        let bytes = [0x00, 0x01, 0x00, 0x00, 0x00, 0x0C];
        assert_eq!(detect_font_format(&bytes), Some(FontFormat::Ttf));
    }

    #[test]
    fn detect_otf_magic() {
        let bytes = [0x4F, 0x54, 0x54, 0x4F, 0x00, 0x01];
        assert_eq!(detect_font_format(&bytes), Some(FontFormat::Otf));
    }

    #[test]
    fn detect_woff_magic() {
        let bytes = [0x77, 0x4F, 0x46, 0x46, 0x00, 0x01];
        assert_eq!(detect_font_format(&bytes), Some(FontFormat::Woff));
    }

    #[test]
    fn detect_woff2_magic() {
        let bytes = [0x77, 0x4F, 0x46, 0x32, 0x00, 0x01];
        assert_eq!(detect_font_format(&bytes), Some(FontFormat::Woff2));
    }

    #[test]
    fn reject_unknown_magic() {
        let bytes = [0x89, 0x50, 0x4E, 0x47]; // PNG
        assert_eq!(detect_font_format(&bytes), None);
    }

    #[test]
    fn reject_too_short() {
        assert_eq!(detect_font_format(&[0x00, 0x01]), None);
    }

    #[test]
    fn detect_from_extension() {
        assert_eq!(
            detect_font_from_extension("font.ttf"),
            Some(FontFormat::Ttf)
        );
        assert_eq!(
            detect_font_from_extension("font.OTF"),
            Some(FontFormat::Otf)
        );
        assert_eq!(
            detect_font_from_extension("font.woff"),
            Some(FontFormat::Woff)
        );
        assert_eq!(
            detect_font_from_extension("font.WOFF2"),
            Some(FontFormat::Woff2)
        );
        assert_eq!(detect_font_from_extension("font.txt"), None);
    }

    #[test]
    fn is_font_filename_checks() {
        assert!(is_font_filename("arial.ttf"));
        assert!(is_font_filename("custom.woff2"));
        assert!(!is_font_filename("image.png"));
        assert!(!is_font_filename("document.pdf"));
    }

    #[test]
    fn family_name_extraction() {
        assert_eq!(family_name_from_filename("Roboto-Bold.ttf"), "Roboto Bold");
        assert_eq!(family_name_from_filename("Fira_Code.otf"), "Fira Code");
        assert_eq!(family_name_from_filename("Arial.ttf"), "Arial");
        assert_eq!(
            family_name_from_filename("My-Font_Name.woff"),
            "My Font Name"
        );
    }

    #[test]
    fn font_format_properties() {
        assert_eq!(FontFormat::Ttf.as_str(), "TrueType");
        assert_eq!(FontFormat::Otf.extension(), "otf");
        assert_eq!(FontFormat::Woff.mime_type(), "font/woff");
        assert_eq!(FontFormat::Woff2.as_str(), "WOFF2");
    }

    #[test]
    fn create_metadata_from_bytes() {
        let bytes = [0x00, 0x01, 0x00, 0x00, 0x00, 0x0C];
        let meta = create_font_metadata("test.ttf", 1024, Some(&bytes)).unwrap();
        assert_eq!(meta.format, FontFormat::Ttf);
        assert_eq!(meta.file_size, 1024);
        assert_eq!(meta.family_name, Some("test".to_string()));
    }

    #[test]
    fn create_metadata_from_extension_only() {
        let meta = create_font_metadata("MyFont.woff2", 2048, None).unwrap();
        assert_eq!(meta.format, FontFormat::Woff2);
        assert_eq!(meta.family_name, Some("MyFont".to_string()));
    }

    #[test]
    fn create_metadata_fails_for_non_font() {
        assert!(create_font_metadata("image.png", 100, None).is_none());
    }
}
