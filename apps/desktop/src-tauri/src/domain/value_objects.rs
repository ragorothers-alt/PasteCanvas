use std::fmt;

/// A UUIDv7-based entity identifier with time-ordering properties.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityId(String);

impl EntityId {
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EntityId> for String {
    fn from(id: EntityId) -> Self {
        id.0
    }
}

/// UTC Unix milliseconds timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixMs(i64);

impl UnixMs {
    pub fn now() -> Self {
        Self(chrono::Utc::now().timestamp_millis())
    }

    pub fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    pub fn as_millis(&self) -> i64 {
        self.0
    }
}

/// Content types supported by PasteCanvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    PlainText,
    RichText,
    Code,
    Image,
    Screenshot,
    Link,
    VideoLink,
    Pdf,
    Color,
    Font,
    Svg,
    File,
}

impl ItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PlainText => "plain_text",
            Self::RichText => "rich_text",
            Self::Code => "code",
            Self::Image => "image",
            Self::Screenshot => "screenshot",
            Self::Link => "link",
            Self::VideoLink => "video_link",
            Self::Pdf => "pdf",
            Self::Color => "color",
            Self::Font => "font",
            Self::Svg => "svg",
            Self::File => "file",
        }
    }
}

impl std::str::FromStr for ItemType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "plain_text" => Ok(Self::PlainText),
            "rich_text" => Ok(Self::RichText),
            "code" => Ok(Self::Code),
            "image" => Ok(Self::Image),
            "screenshot" => Ok(Self::Screenshot),
            "link" => Ok(Self::Link),
            "video_link" => Ok(Self::VideoLink),
            "pdf" => Ok(Self::Pdf),
            "color" => Ok(Self::Color),
            "font" => Ok(Self::Font),
            "svg" => Ok(Self::Svg),
            "file" => Ok(Self::File),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_generates_unique_values() {
        let id1 = EntityId::new();
        let id2 = EntityId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn unix_ms_now_is_positive() {
        let ts = UnixMs::now();
        assert!(ts.as_millis() > 0);
    }

    #[test]
    fn item_type_round_trips() {
        let types = [
            ItemType::PlainText,
            ItemType::Image,
            ItemType::Link,
            ItemType::Pdf,
        ];
        for t in types {
            let s = t.as_str();
            let parsed: Result<ItemType, ()> = s.parse();
            assert_eq!(Ok(t), parsed);
        }
    }

    #[test]
    fn item_type_unknown_returns_err() {
        let result: Result<ItemType, ()> = "unknown_type".parse();
        assert!(result.is_err());
    }
}
