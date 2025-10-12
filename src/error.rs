//! Error types for ryt

use thiserror::Error;

/// Main error type for ryt operations
#[derive(Debug, Error)]
pub enum RytError {
    #[error("Video is geo-blocked")]
    GeoBlocked,
    
    #[error("Rate limited")]
    RateLimited,
    
    #[error("Age restricted")]
    AgeRestricted,
    
    #[error("Private video")]
    Private,
    
    #[error("Video unavailable")]
    VideoUnavailable,
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("No suitable format found")]
    NoFormatFound,
    
    #[error("Download failed: {0}")]
    DownloadFailed(#[from] reqwest::Error),
    
    #[error("Parse error: {0}")]
    ParseError(#[from] std::num::ParseIntError),
    
    #[error("API key not found")]
    ApiKeyNotFound,
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),
    
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
    
    #[error("Botguard error: {0}")]
    BotguardError(String),
    
    #[error("Cipher error: {0}")]
    CipherError(String),
    
    #[error("Format parsing error: {0}")]
    FormatError(String),
    
    #[error("Playlist error: {0}")]
    PlaylistError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Rate limit error: {0}")]
    RateLimitError(String),
    
    #[error("Generic error: {0}")]
    Generic(String),
}

impl RytError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            RytError::DownloadFailed(_) | 
            RytError::TimeoutError(_) | 
            RytError::RateLimited |
            RytError::AgeRestricted
        )
    }
    
    /// Check if error is a YouTube-specific error
    pub fn is_youtube_error(&self) -> bool {
        matches!(
            self,
            RytError::GeoBlocked |
            RytError::RateLimited |
            RytError::AgeRestricted |
            RytError::Private |
            RytError::VideoUnavailable
        )
    }
}
