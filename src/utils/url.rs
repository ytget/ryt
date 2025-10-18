//! URL utilities for extracting video IDs and parsing video platform URLs

use crate::error::RytError;
use url::Url;

/// Extract video ID from various video platform URL formats
pub fn extract_video_id(url: &str) -> Result<String, RytError> {
    let parsed = Url::parse(url)?;

    match parsed.host_str() {
        Some("youtu.be") => {
            let path = parsed.path().trim_start_matches('/');
            if path.is_empty() {
                return Err(RytError::InvalidUrl("Missing video ID".to_string()));
            }
            Ok(path.to_string())
        }
        Some("youtube.com") | Some("www.youtube.com") => {
            if parsed.path().starts_with("/watch") {
                parsed
                    .query_pairs()
                    .find(|(key, _)| key == "v")
                    .map(|(_, value)| value.to_string())
                    .ok_or_else(|| RytError::InvalidUrl("Missing v parameter".to_string()))
            } else if parsed.path().starts_with("/shorts/") {
                let video_id = parsed.path().trim_start_matches("/shorts/");
                if video_id.is_empty() {
                    return Err(RytError::InvalidUrl(
                        "Missing video ID in shorts path".to_string(),
                    ));
                }
                Ok(video_id.to_string())
            } else {
                Err(RytError::InvalidUrl(
                    "Unsupported video URL format".to_string(),
                ))
            }
        }
        _ => Err(RytError::InvalidUrl(
            "Not a supported video platform URL".to_string(),
        )),
    }
}

/// Extract playlist ID from video platform playlist URL
pub fn extract_playlist_id(url: &str) -> Result<String, RytError> {
    // Accept raw playlist IDs as-is
    if !url.is_empty()
        && (url.starts_with("PL") || url.starts_with("UU") || url.starts_with("OLAK5uy_"))
    {
        return Ok(url.to_string());
    }

    let parsed = Url::parse(url)?;
    if let Some(id) = parsed
        .query_pairs()
        .find(|(key, _)| key == "list")
        .map(|(_, value)| value.to_string())
    {
        Ok(id)
    } else {
        Err(RytError::InvalidUrl("Playlist ID not found".to_string()))
    }
}

/// Check if URL is a supported video platform URL
pub fn is_video_url(url: &str) -> bool {
    if let Ok(parsed) = Url::parse(url) {
        matches!(
            parsed.host_str(),
            Some("youtube.com") | Some("www.youtube.com") | Some("youtu.be")
        )
    } else {
        false
    }
}

/// Check if URL is a playlist URL
pub fn is_playlist_url(url: &str) -> bool {
    if let Ok(parsed) = Url::parse(url) {
        parsed.path().contains("/playlist") || parsed.query_pairs().any(|(key, _)| key == "list")
    } else {
        // If URL parsing fails, check if it's a raw playlist ID
        url.starts_with("PL") || url.starts_with("UU") || url.starts_with("OLAK5uy_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_video_id() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );

        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );

        assert_eq!(
            extract_video_id("https://www.youtube.com/shorts/brZCOVlyPPo").unwrap(),
            "brZCOVlyPPo"
        );

        // Test error cases
        assert!(extract_video_id("https://www.youtube.com/watch").is_err());
        assert!(extract_video_id("https://example.com").is_err());
    }

    #[test]
    fn test_extract_playlist_id() {
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/playlist?list=PLxxxx").unwrap(),
            "PLxxxx"
        );

        assert_eq!(extract_playlist_id("PLxxxx").unwrap(), "PLxxxx");

        assert!(extract_playlist_id("https://www.youtube.com/watch?v=xxx").is_err());
    }

    #[test]
    fn test_is_video_url() {
        assert!(is_video_url("https://www.youtube.com/watch?v=xxx"));
        assert!(is_video_url("https://youtu.be/xxx"));
        assert!(!is_video_url("https://example.com"));
    }

    #[test]
    fn test_is_playlist_url() {
        assert!(is_playlist_url(
            "https://www.youtube.com/playlist?list=PLxxxx"
        ));
        assert!(is_playlist_url("PLxxxx"));
        assert!(!is_playlist_url("https://www.youtube.com/watch?v=xxx"));
    }

    #[test]
    fn test_extract_video_id_comprehensive() {
        // Test youtu.be URLs
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ?t=10").unwrap(),
            "dQw4w9WgXcQ"
        );

        // Test youtube.com URLs
        assert_eq!(
            extract_video_id("https://youtube.com/watch?v=dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );

        // Test shorts URLs
        assert_eq!(
            extract_video_id("https://www.youtube.com/shorts/brZCOVlyPPo").unwrap(),
            "brZCOVlyPPo"
        );
        assert_eq!(
            extract_video_id("https://youtube.com/shorts/brZCOVlyPPo").unwrap(),
            "brZCOVlyPPo"
        );

        // Test error cases
        assert!(extract_video_id("https://youtu.be/").is_err());
        assert!(extract_video_id("https://www.youtube.com/watch").is_err());
        assert!(extract_video_id("https://www.youtube.com/shorts/").is_err());
        assert!(extract_video_id("https://www.youtube.com/channel/UCxxx").is_err());
        assert!(extract_video_id("https://example.com").is_err());
        assert!(extract_video_id("not-a-url").is_err());
    }

    #[test]
    fn test_extract_playlist_id_comprehensive() {
        // Test raw playlist IDs
        assert_eq!(extract_playlist_id("PLxxxx").unwrap(), "PLxxxx");
        assert_eq!(extract_playlist_id("UUxxxx").unwrap(), "UUxxxx");
        assert_eq!(extract_playlist_id("OLAK5uy_xxxx").unwrap(), "OLAK5uy_xxxx");

        // Test playlist URLs
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/playlist?list=PLxxxx").unwrap(),
            "PLxxxx"
        );
        assert_eq!(
            extract_playlist_id("https://youtube.com/playlist?list=UUxxxx").unwrap(),
            "UUxxxx"
        );

        // Test error cases
        assert!(extract_playlist_id("https://www.youtube.com/watch?v=xxx").is_err());
        assert!(extract_playlist_id("").is_err());
        assert!(extract_playlist_id("not-a-url").is_err());
    }

    #[test]
    fn test_is_video_url_comprehensive() {
        // Test supported video URLs
        assert!(is_video_url("https://www.youtube.com/watch?v=xxx"));
        assert!(is_video_url("https://youtube.com/watch?v=xxx"));
        assert!(is_video_url("https://youtu.be/xxx"));
        assert!(is_video_url("https://www.youtube.com/shorts/xxx"));
        assert!(is_video_url("https://youtube.com/shorts/xxx"));

        // Test unsupported URLs
        assert!(!is_video_url("https://example.com"));
        assert!(!is_video_url("https://vimeo.com/xxx"));
        assert!(!is_video_url("not-a-url"));
        assert!(!is_video_url(""));
    }

    #[test]
    fn test_is_playlist_url_comprehensive() {
        // Test playlist URLs
        assert!(is_playlist_url(
            "https://www.youtube.com/playlist?list=PLxxxx"
        ));
        assert!(is_playlist_url("https://youtube.com/playlist?list=UUxxxx"));
        assert!(is_playlist_url(
            "https://www.youtube.com/watch?v=xxx&list=PLxxxx"
        ));

        // Test raw playlist IDs
        assert!(is_playlist_url("PLxxxx"));
        assert!(is_playlist_url("UUxxxx"));
        assert!(is_playlist_url("OLAK5uy_xxxx"));

        // Test non-playlist URLs
        assert!(!is_playlist_url("https://www.youtube.com/watch?v=xxx"));
        assert!(!is_playlist_url("https://example.com"));
        assert!(!is_playlist_url("not-a-url"));
        assert!(!is_playlist_url(""));

        // Test invalid URLs (should still work for raw IDs)
        assert!(is_playlist_url("PLxxxx")); // Raw ID should work even if URL parsing fails
    }

    #[test]
    fn test_extract_video_id_edge_cases() {
        // Test URLs with additional parameters
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s").unwrap(),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ?t=10s&list=PLxxxx").unwrap(),
            "dQw4w9WgXcQ"
        );

        // Test URLs with fragments
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=10s").unwrap(),
            "dQw4w9WgXcQ"
        );

        // Test case sensitivity
        assert_eq!(
            extract_video_id("https://YOUTU.BE/dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            extract_video_id("https://YOUTUBE.COM/watch?v=dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_extract_playlist_id_edge_cases() {
        // Test URLs with additional parameters
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/playlist?list=PLxxxx&index=1").unwrap(),
            "PLxxxx"
        );
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/watch?v=xxx&list=PLxxxx&t=10s").unwrap(),
            "PLxxxx"
        );

        // Test case sensitivity for raw IDs (these should work as raw IDs)
        // Note: Only uppercase prefixes are supported for raw IDs
        assert_eq!(extract_playlist_id("PLxxxx").unwrap(), "PLxxxx");
        assert_eq!(extract_playlist_id("UUxxxx").unwrap(), "UUxxxx");
    }
}
