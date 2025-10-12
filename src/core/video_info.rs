//! Video information structures

use serde::{Deserialize, Serialize};

/// Video information and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    /// YouTube video ID
    pub id: String,
    /// Video title
    pub title: String,
    /// Video author/channel name
    pub author: String,
    /// Video duration in seconds
    pub duration: u32,
    /// Video description
    pub description: String,
    /// Available formats
    pub formats: Vec<Format>,
    /// Video thumbnail URL
    pub thumbnail: Option<String>,
    /// Video upload date
    pub upload_date: Option<String>,
    /// Video view count
    pub view_count: Option<u64>,
    /// Video like count
    pub like_count: Option<u64>,
    /// Video tags
    pub tags: Vec<String>,
    /// Video category
    pub category: Option<String>,
}

impl VideoInfo {
    /// Create a new VideoInfo
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            author: String::new(),
            duration: 0,
            description: String::new(),
            formats: Vec::new(),
            thumbnail: None,
            upload_date: None,
            view_count: None,
            like_count: None,
            tags: Vec::new(),
            category: None,
        }
    }

    /// Get the best available format
    pub fn best_format(&self) -> Option<&Format> {
        self.formats.iter().max_by_key(|f| f.bitrate)
    }

    /// Get formats filtered by extension
    pub fn formats_by_extension(&self, extension: &str) -> Vec<&Format> {
        self.formats
            .iter()
            .filter(|f| f.mime_type.contains(extension))
            .collect()
    }

    /// Get formats filtered by quality
    pub fn formats_by_quality(&self, quality: &str) -> Vec<&Format> {
        self.formats
            .iter()
            .filter(|f| f.quality == quality)
            .collect()
    }

    /// Get the total size of all formats
    pub fn total_size(&self) -> u64 {
        self.formats.iter().map(|f| f.size.unwrap_or(0)).sum()
    }

    /// Check if video has progressive formats (video+audio combined)
    pub fn has_progressive_formats(&self) -> bool {
        self.formats.iter().any(|f| f.is_progressive())
    }

    /// Check if video has adaptive formats (video or audio only)
    pub fn has_adaptive_formats(&self) -> bool {
        self.formats.iter().any(|f| f.is_adaptive())
    }
}

/// Video format information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Format {
    /// YouTube format ID (itag)
    pub itag: u32,
    /// Direct download URL
    pub url: String,
    /// Quality label (e.g., "720p", "1080p")
    pub quality: String,
    /// MIME type
    pub mime_type: String,
    /// Bitrate in bits per second
    pub bitrate: u32,
    /// File size in bytes (if known)
    pub size: Option<u64>,
    /// Signature cipher (if encrypted)
    pub signature_cipher: Option<String>,
    /// Audio codec
    pub audio_codec: Option<String>,
    /// Video codec
    pub video_codec: Option<String>,
    /// Frame rate
    pub fps: Option<u32>,
    /// Video width
    pub width: Option<u32>,
    /// Video height
    pub height: Option<u32>,
    /// Audio sample rate
    pub audio_sample_rate: Option<u32>,
    /// Audio channels
    pub audio_channels: Option<u32>,
    /// Language code
    pub language: Option<String>,
    /// Format note/description
    pub note: Option<String>,
}

impl Format {
    /// Create a new Format
    pub fn new(itag: u32, url: String, quality: String, mime_type: String) -> Self {
        Self {
            itag,
            url,
            quality,
            mime_type,
            bitrate: 0,
            size: None,
            signature_cipher: None,
            audio_codec: None,
            video_codec: None,
            fps: None,
            width: None,
            height: None,
            audio_sample_rate: None,
            audio_channels: None,
            language: None,
            note: None,
        }
    }

    /// Check if format is progressive (video+audio combined)
    pub fn is_progressive(&self) -> bool {
        self.mime_type.starts_with("video/")
            && (self.mime_type.contains("mp4") || self.mime_type.contains("webm"))
            && self.audio_codec.is_some()
            && self.video_codec.is_some()
    }

    /// Check if format is adaptive (video or audio only)
    pub fn is_adaptive(&self) -> bool {
        (self.mime_type.starts_with("video/") && self.audio_codec.is_none())
            || self.mime_type.starts_with("audio/")
    }

    /// Check if format is video-only
    pub fn is_video_only(&self) -> bool {
        self.mime_type.starts_with("video/") && self.audio_codec.is_none()
    }

    /// Check if format is audio-only
    pub fn is_audio_only(&self) -> bool {
        self.mime_type.starts_with("audio/")
    }

    /// Get file extension from MIME type
    pub fn extension(&self) -> &'static str {
        crate::utils::mime::ext_from_mime(&self.mime_type)
    }

    /// Get container format
    pub fn container(&self) -> &'static str {
        crate::utils::mime::get_container_format(&self.mime_type)
    }

    /// Check if format needs signature deciphering
    pub fn needs_deciphering(&self) -> bool {
        self.signature_cipher.is_some()
            || self.url.contains("&n=")
            || self.url.contains("?n=")
            || self.url.is_empty()
    }

    /// Get human-readable quality string
    pub fn quality_string(&self) -> String {
        if !self.quality.is_empty() {
            self.quality.clone()
        } else if let (Some(width), Some(height)) = (self.width, self.height) {
            format!("{}x{}", width, height)
        } else {
            "Unknown".to_string()
        }
    }

    /// Get human-readable size string
    pub fn size_string(&self) -> String {
        if let Some(size) = self.size {
            crate::core::progress::format_bytes(size)
        } else {
            "Unknown".to_string()
        }
    }

    /// Get human-readable bitrate string
    pub fn bitrate_string(&self) -> String {
        if self.bitrate > 0 {
            format!("{} kbps", self.bitrate / 1000)
        } else {
            "Unknown".to_string()
        }
    }
}

/// Playlist item information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    /// Video ID
    pub video_id: String,
    /// Video title
    pub title: String,
    /// Video author
    pub author: String,
    /// Video duration in seconds
    pub duration: u32,
    /// Playlist index
    pub index: u32,
    /// Video thumbnail URL
    pub thumbnail: Option<String>,
    /// Video description
    pub description: Option<String>,
}

impl PlaylistItem {
    /// Create a new PlaylistItem
    pub fn new(video_id: String, title: String, index: u32) -> Self {
        Self {
            video_id,
            title,
            author: String::new(),
            duration: 0,
            index,
            thumbnail: None,
            description: None,
        }
    }

    /// Get the YouTube URL for this video
    pub fn url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.video_id)
    }
}

/// Format selector for choosing video formats
#[derive(Debug, Clone)]
pub struct FormatSelector {
    /// Quality selector
    pub quality: QualitySelector,
    /// Desired file extension
    pub extension: Option<String>,
    /// Maximum height constraint
    pub height_limit: Option<u32>,
    /// Minimum height constraint
    pub height_min: Option<u32>,
    /// Preferred itag
    pub preferred_itag: Option<u32>,
}

impl FormatSelector {
    /// Create a new format selector
    pub fn new(quality: QualitySelector) -> Self {
        Self {
            quality,
            extension: None,
            height_limit: None,
            height_min: None,
            preferred_itag: None,
        }
    }

    /// Set desired extension
    pub fn with_extension(mut self, extension: &str) -> Self {
        self.extension = Some(extension.to_string());
        self
    }

    /// Set height limit
    pub fn with_height_limit(mut self, height: u32) -> Self {
        self.height_limit = Some(height);
        self
    }

    /// Set minimum height
    pub fn with_height_min(mut self, height: u32) -> Self {
        self.height_min = Some(height);
        self
    }

    /// Set preferred itag
    pub fn with_itag(mut self, itag: u32) -> Self {
        self.preferred_itag = Some(itag);
        self
    }
}

/// Quality selection criteria
#[derive(Debug, Clone, PartialEq)]
pub enum QualitySelector {
    /// Best quality available
    Best,
    /// Worst quality available
    Worst,
    /// Specific itag
    Itag(u32),
    /// Specific height
    Height(u32),
    /// Height less than or equal to
    HeightLessOrEqual(u32),
    /// Height greater than or equal to
    HeightGreaterOrEqual(u32),
}

impl QualitySelector {
    /// Parse quality selector from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim().to_lowercase();

        match s.as_str() {
            "best" => Ok(QualitySelector::Best),
            "worst" => Ok(QualitySelector::Worst),
            _ => {
                if s.starts_with("itag=") {
                    let itag_str = &s[5..];
                    let itag = itag_str
                        .parse::<u32>()
                        .map_err(|_| format!("Invalid itag: {}", itag_str))?;
                    Ok(QualitySelector::Itag(itag))
                } else if s.starts_with("height<=") {
                    let height_str = &s[8..];
                    let height = height_str
                        .parse::<u32>()
                        .map_err(|_| format!("Invalid height: {}", height_str))?;
                    Ok(QualitySelector::HeightLessOrEqual(height))
                } else if s.starts_with("height>=") {
                    let height_str = &s[8..];
                    let height = height_str
                        .parse::<u32>()
                        .map_err(|_| format!("Invalid height: {}", height_str))?;
                    Ok(QualitySelector::HeightGreaterOrEqual(height))
                } else if s.starts_with("height=") {
                    let height_str = &s[7..];
                    let height = height_str
                        .parse::<u32>()
                        .map_err(|_| format!("Invalid height: {}", height_str))?;
                    Ok(QualitySelector::Height(height))
                } else {
                    Err(format!("Unknown quality selector: {}", s))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_info_creation() {
        let info = VideoInfo::new("test_id".to_string(), "Test Video".to_string());
        assert_eq!(info.id, "test_id");
        assert_eq!(info.title, "Test Video");
        assert!(info.formats.is_empty());
    }

    #[test]
    fn test_format_creation() {
        let format = Format::new(
            22,
            "http://example.com".to_string(),
            "720p".to_string(),
            "video/mp4".to_string(),
        );
        assert_eq!(format.itag, 22);
        assert_eq!(format.quality, "720p");
        assert_eq!(format.mime_type, "video/mp4");
    }

    #[test]
    fn test_format_progressive() {
        let mut format = Format::new(
            22,
            "http://example.com".to_string(),
            "720p".to_string(),
            "video/mp4".to_string(),
        );
        format.audio_codec = Some("aac".to_string());
        format.video_codec = Some("avc1".to_string());

        assert!(format.is_progressive());
        assert!(!format.is_adaptive());
    }

    #[test]
    fn test_format_adaptive() {
        let format = Format::new(
            137,
            "http://example.com".to_string(),
            "1080p".to_string(),
            "video/mp4".to_string(),
        );
        assert!(!format.is_progressive());
        assert!(format.is_adaptive());
        assert!(format.is_video_only());
    }

    #[test]
    fn test_quality_selector_parsing() {
        assert_eq!(
            QualitySelector::from_str("best").unwrap(),
            QualitySelector::Best
        );
        assert_eq!(
            QualitySelector::from_str("worst").unwrap(),
            QualitySelector::Worst
        );
        assert_eq!(
            QualitySelector::from_str("itag=22").unwrap(),
            QualitySelector::Itag(22)
        );
        assert_eq!(
            QualitySelector::from_str("height<=720").unwrap(),
            QualitySelector::HeightLessOrEqual(720)
        );
        assert_eq!(
            QualitySelector::from_str("height>=480").unwrap(),
            QualitySelector::HeightGreaterOrEqual(480)
        );
        assert_eq!(
            QualitySelector::from_str("height=1080").unwrap(),
            QualitySelector::Height(1080)
        );

        assert!(QualitySelector::from_str("invalid").is_err());
    }

    #[test]
    fn test_format_selector() {
        let selector = FormatSelector::new(QualitySelector::Best)
            .with_extension("mp4")
            .with_height_limit(720);

        assert!(matches!(selector.quality, QualitySelector::Best));
        assert_eq!(selector.extension, Some("mp4".to_string()));
        assert_eq!(selector.height_limit, Some(720));
    }
}
