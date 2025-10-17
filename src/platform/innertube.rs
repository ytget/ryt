//! InnerTube API client for video platform

use crate::core::video_info::{Format, PlaylistItem};
use crate::error::RytError;
use crate::platform::client::VideoClient;
use regex::Regex;
use serde::Deserialize;
use tracing::{debug, info, warn};

/// InnerTube API client
pub struct InnerTubeClient {
    http_client: VideoClient,
    client_name: String,
    client_version: String,
    api_key: Option<String>,
    visitor_id: Option<String>,
}

impl InnerTubeClient {
    /// Create a new InnerTube client
    pub fn new() -> Self {
        Self {
            http_client: VideoClient::new(),
            client_name: "ANDROID".to_string(), // ANDROID gives direct URLs
            client_version: "20.10.38".to_string(),
            api_key: None,
            visitor_id: None,
        }
    }

    /// Set client name and version
    pub fn with_client(mut self, name: &str, version: &str) -> Self {
        self.client_name = name.to_string();
        self.client_version = version.to_string();
        self
    }

    /// Set visitor ID
    pub fn with_visitor_id(mut self, visitor_id: &str) -> Self {
        self.visitor_id = Some(visitor_id.to_string());
        self
    }

    /// Switch client for error handling
    pub fn switch_client_for_error(&mut self, error: &RytError) {
        self.http_client.switch_client_by_strategy(Some(error));
    }

    /// Extract API key and client version from YouTube HTML
    async fn ensure_api_key(&mut self, video_id: &str) -> Result<(), RytError> {
        if self.api_key.is_some() {
            return Ok(());
        }

        info!("Extracting API key and client version from YouTube HTML");

        // Try multiple sources for API key and client version
        let sources = vec![
            format!("https://www.youtube.com/watch?v={}", video_id),
            "https://www.youtube.com".to_string(),
            "https://www.youtube.com/feed/trending".to_string(),
            "https://www.youtube.com/feed/explore".to_string(),
        ];

        let api_key_regex = Regex::new(r#""INNERTUBE_API_KEY":"([^"]+)""#)?;
        let client_ver_regex = Regex::new(r#""INNERTUBE_CLIENT_VERSION":"([^"]+)""#)?;

        for source in sources {
            if self.api_key.is_some() {
                break;
            }

            debug!("Trying to extract API key from: {}", source);

            let response = self
                .http_client
                .create_realistic_request(reqwest::Method::GET, &source)
                .send()
                .await?;

            if !response.status().is_success() {
                warn!("Failed to fetch {}: {}", source, response.status());
                continue;
            }

            let body = response.text().await?;

            // Extract API key if not found yet
            if self.api_key.is_none() {
                if let Some(captures) = api_key_regex.captures(&body) {
                    if let Some(api_key) = captures.get(1) {
                        self.api_key = Some(api_key.as_str().to_string());
                        info!("Extracted API key: {}...", &api_key.as_str()[..10]);
                    }
                }
            }

            // Extract client version if not found yet
            if let Some(captures) = client_ver_regex.captures(&body) {
                if let Some(client_ver) = captures.get(1) {
                    self.client_version = client_ver.as_str().to_string();
                    info!("Extracted client version: {}", client_ver.as_str());
                }
            }
        }

        if self.api_key.is_none() {
            return Err(RytError::ApiKeyNotFound);
        }

        Ok(())
    }

    /// Get player response for a video
    pub async fn get_player_response(
        &mut self,
        video_id: &str,
    ) -> Result<PlayerResponse, RytError> {
        info!("Fetching player response for video ID: {}", video_id);

        // Ensure we have an API key
        self.ensure_api_key(video_id).await?;

        // Build client context based on client type
        let client_context = if self.client_name == "ANDROID" {
            serde_json::json!({
                "clientName": "ANDROID",
                "clientVersion": "20.10.38",
                "androidSdkVersion": 30,
                "osName": "Android",
                "osVersion": "11",
                "userAgent": "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip"
            })
        } else {
            serde_json::json!({
                "clientName": self.client_name,
                "clientVersion": self.client_version,
                "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "mainAppWebInfo": {
                    "graftUrl": format!("https://www.youtube.com/watch?v={}", video_id),
                    "webDisplayMode": "WEB_DISPLAY_MODE_BROWSER",
                    "isWebNativeShareEnabled": true
                }
            })
        };

        let request_body = serde_json::json!({
            "context": {
                "client": client_context
            },
            "videoId": video_id
        });

        let api_key = self.api_key.as_ref().unwrap();
        let url = format!("https://www.youtube.com/youtubei/v1/player?key={}", api_key);

        debug!("Using API key: {}...", &api_key[..10]);
        debug!("Request URL: {}", url);
        debug!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body).unwrap_or_default()
        );

        let mut request = self.http_client.create_innertube_request(&url);

        // Add Android-specific headers
        if self.client_name == "ANDROID" {
            request = request
                .header("X-YouTube-Client-Name", "3")
                .header("X-YouTube-Client-Version", "20.10.38")
                .header(
                    "User-Agent",
                    "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip",
                );
        }

        if let Some(visitor_id) = &self.visitor_id {
            request = request.header("x-goog-visitor-id", visitor_id);
        }

        let response: PlayerResponse = self
            .http_client
            .execute_with_retry(request.json(&request_body))
            .await?;

        debug!("Player response received successfully");

        // Check playability status
        if let Some(playability_status) = &response.playability_status {
            match playability_status.status.as_str() {
                "ERROR" => {
                    if let Some(reason) = &playability_status.reason {
                        warn!("Video playability error: {}", reason);
                        let reason_lower = reason.to_lowercase();
                        if reason_lower.contains("geograph")
                            || reason_lower.contains("available in your country")
                        {
                            return Err(RytError::GeoBlocked);
                        }
                        if reason_lower.contains("rate limit") || reason_lower.contains("quota") {
                            return Err(RytError::RateLimited);
                        }
                        Err(RytError::VideoUnavailable)
                    } else {
                        warn!("Video playability error: unknown reason");
                        Err(RytError::VideoUnavailable)
                    }
                }
                "LOGIN_REQUIRED" => {
                    warn!("Age restriction detected, this may require client switching");
                    Err(RytError::AgeRestricted)
                }
                "UNPLAYABLE" => {
                    if let Some(reason) = &playability_status.reason {
                        let reason_lower = reason.to_lowercase();
                        if reason_lower.contains("private") {
                            Err(RytError::Private)
                        } else {
                            Err(RytError::VideoUnavailable)
                        }
                    } else {
                        Err(RytError::VideoUnavailable)
                    }
                }
                _ => Ok(response),
            }
        } else {
            // No playability status, assume OK
            Ok(response)
        }
    }

    /// Get playlist items
    pub async fn get_playlist_items(
        &mut self,
        playlist_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<PlaylistItem>, RytError> {
        let request_body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": self.client_name,
                    "clientVersion": self.client_version,
                    "androidSdkVersion": 30,
                    "osName": "Android",
                    "osVersion": "11",
                    "deviceModel": "SM-G973F",
                    "userAgent": format!("com.google.android.youtube/{} (Linux; U; Android 11) gzip", self.client_version),
                    "connectionType": "WIFI",
                    "memoryTotalKb": 4194304
                }
            },
            "browseId": format!("VL{}", playlist_id),
            "params": "6gPTAUNwc0RRUXh4Zz09"
        });

        let mut request = self
            .http_client
            .create_innertube_request("https://www.youtube.com/youtubei/v1/browse");

        if let Some(visitor_id) = &self.visitor_id {
            request = request.header("x-goog-visitor-id", visitor_id);
        }

        let response: BrowseResponse = self
            .http_client
            .execute_with_retry(request.json(&request_body))
            .await?;

        // Parse playlist items from response
        let mut items = Vec::new();
        if let Some(contents) = response
            .contents
            .two_column_browse_results_renderer
            .tabs
            .first()
        {
            if let Some(playlist) = &contents
                .tab_renderer
                .content
                .section_list_renderer
                .contents
                .first()
            {
                if let Some(items_renderer) = &playlist.item_section_renderer.contents.first() {
                    let playlist_video_list = &items_renderer.playlist_video_list_renderer;
                    for (index, content) in playlist_video_list.contents.iter().enumerate() {
                        let video = &content.playlist_video_renderer;
                        items.push(PlaylistItem {
                            video_id: video.video_id.clone(),
                            title: video
                                .title
                                .runs
                                .first()
                                .map(|r| r.text.clone())
                                .unwrap_or_default(),
                            author: video
                                .short_byline_text
                                .runs
                                .first()
                                .map(|r| r.text.clone())
                                .unwrap_or_default(),
                            duration: video.length_seconds.parse().unwrap_or(0),
                            index: index as u32,
                            thumbnail: video.thumbnail.thumbnails.first().map(|t| t.url.clone()),
                            description: None,
                        });

                        if let Some(limit) = limit {
                            if items.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(items)
    }

    /// Get visitor ID from YouTube main page
    pub async fn get_visitor_id(&self) -> Result<String, RytError> {
        let response = self
            .http_client
            .create_request(reqwest::Method::GET, "https://www.youtube.com")
            .send()
            .await?;

        let html = response.text().await?;

        // Extract visitor ID from ytcfg
        if let Some(start) = html.find("ytcfg.set(") {
            if let Some(end) = html[start..].find("});") {
                let config_str = &html[start + 10..start + end + 1];
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(config_str) {
                    if let Some(visitor_data) =
                        config["INNERTUBE_CONTEXT"]["client"]["visitorData"].as_str()
                    {
                        return Ok(visitor_data.to_string());
                    }
                }
            }
        }

        Err(RytError::Generic(
            "Failed to extract visitor ID".to_string(),
        ))
    }
}

impl Default for InnerTubeClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Player response from InnerTube API
#[derive(Debug, Deserialize)]
pub struct PlayerResponse {
    #[serde(rename = "responseContext")]
    pub response_context: Option<ResponseContext>,
    #[serde(rename = "playabilityStatus")]
    pub playability_status: Option<PlayabilityStatus>,
    #[serde(rename = "videoDetails")]
    pub video_details: Option<VideoDetails>,
    #[serde(rename = "streamingData")]
    pub streaming_data: Option<StreamingData>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseContext {
    #[serde(rename = "visitorData")]
    pub visitor_data: Option<String>,
    #[serde(rename = "serviceTrackingParams")]
    pub service_tracking_params: Option<Vec<ServiceTrackingParam>>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceTrackingParam {
    pub service: String,
    #[serde(rename = "params")]
    pub params: Vec<Param>,
}

#[derive(Debug, Deserialize)]
pub struct Param {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct PlayabilityStatus {
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VideoDetails {
    #[serde(rename = "videoId")]
    pub video_id: String,
    pub title: String,
    pub author: String,
    #[serde(rename = "lengthSeconds")]
    pub length_seconds: String,
    #[serde(rename = "shortDescription")]
    pub short_description: String,
    pub thumbnail: Thumbnail,
}

#[derive(Debug, Deserialize)]
pub struct Thumbnail {
    pub thumbnails: Vec<ThumbnailInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ThumbnailInfo {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
pub struct StreamingData {
    pub formats: Option<Vec<FormatData>>,
    #[serde(rename = "adaptiveFormats")]
    pub adaptive_formats: Option<Vec<FormatData>>,
}

#[derive(Debug, Deserialize)]
pub struct FormatData {
    pub itag: u32,
    pub url: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub bitrate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "qualityLabel")]
    pub quality_label: Option<String>,
    #[serde(rename = "contentLength")]
    pub content_length: Option<String>,
    #[serde(rename = "signatureCipher")]
    pub signature_cipher: Option<String>,
    #[serde(rename = "audioCodec")]
    pub audio_codec: Option<String>,
    #[serde(rename = "videoCodec")]
    pub video_codec: Option<String>,
    pub fps: Option<u32>,
    #[serde(rename = "audioSampleRate")]
    pub audio_sample_rate: Option<serde_json::Value>,
    #[serde(rename = "audioChannels")]
    pub audio_channels: Option<serde_json::Value>,
}

impl PlayerResponse {
    /// Parse formats from player response
    pub fn parse_formats(&self) -> Result<Vec<Format>, RytError> {
        let mut formats = Vec::new();

        // Parse progressive formats
        if let Some(streaming_data) = &self.streaming_data {
            if let Some(formats_data) = &streaming_data.formats {
                for format_data in formats_data {
                    formats.push(Format {
                        itag: format_data.itag,
                        url: format_data.url.clone().unwrap_or_default(),
                        quality: format_data.quality_label.clone().unwrap_or_default(),
                        mime_type: format_data.mime_type.clone(),
                        bitrate: format_data.bitrate.unwrap_or(0),
                        size: format_data
                            .content_length
                            .as_ref()
                            .and_then(|s| s.parse().ok()),
                        signature_cipher: format_data.signature_cipher.clone(),
                        audio_codec: format_data.audio_codec.clone(),
                        video_codec: format_data.video_codec.clone(),
                        fps: format_data.fps,
                        width: format_data.width,
                        height: format_data.height,
                        audio_sample_rate: format_data.audio_sample_rate.as_ref().and_then(|v| {
                            v.as_str()
                                .and_then(|s| s.parse().ok())
                                .or_else(|| v.as_u64().map(|n| n as u32))
                        }),
                        audio_channels: format_data.audio_channels.as_ref().and_then(|v| {
                            v.as_str()
                                .and_then(|s| s.parse().ok())
                                .or_else(|| v.as_u64().map(|n| n as u32))
                        }),
                        language: None,
                        note: None,
                    });
                }
            }

            // Parse adaptive formats
            if let Some(adaptive_formats) = &streaming_data.adaptive_formats {
                for format_data in adaptive_formats {
                    formats.push(Format {
                        itag: format_data.itag,
                        url: format_data.url.clone().unwrap_or_default(),
                        quality: format_data.quality_label.clone().unwrap_or_default(),
                        mime_type: format_data.mime_type.clone(),
                        bitrate: format_data.bitrate.unwrap_or(0),
                        size: format_data
                            .content_length
                            .as_ref()
                            .and_then(|s| s.parse().ok()),
                        signature_cipher: format_data.signature_cipher.clone(),
                        audio_codec: format_data.audio_codec.clone(),
                        video_codec: format_data.video_codec.clone(),
                        fps: format_data.fps,
                        width: format_data.width,
                        height: format_data.height,
                        audio_sample_rate: format_data.audio_sample_rate.as_ref().and_then(|v| {
                            v.as_str()
                                .and_then(|s| s.parse().ok())
                                .or_else(|| v.as_u64().map(|n| n as u32))
                        }),
                        audio_channels: format_data.audio_channels.as_ref().and_then(|v| {
                            v.as_str()
                                .and_then(|s| s.parse().ok())
                                .or_else(|| v.as_u64().map(|n| n as u32))
                        }),
                        language: None,
                        note: None,
                    });
                }
            }
        }

        // If no formats found, return error
        if formats.is_empty() {
            return Err(RytError::NoFormatFound);
        }

        Ok(formats)
    }
}

/// Browse response for playlists
#[derive(Debug, Deserialize)]
pub struct BrowseResponse {
    pub contents: BrowseContents,
}

#[derive(Debug, Deserialize)]
pub struct BrowseContents {
    pub two_column_browse_results_renderer: TwoColumnBrowseResultsRenderer,
}

#[derive(Debug, Deserialize)]
pub struct TwoColumnBrowseResultsRenderer {
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Deserialize)]
pub struct Tab {
    pub tab_renderer: TabRenderer,
}

#[derive(Debug, Deserialize)]
pub struct TabRenderer {
    pub content: TabContent,
}

#[derive(Debug, Deserialize)]
pub struct TabContent {
    pub section_list_renderer: SectionListRenderer,
}

#[derive(Debug, Deserialize)]
pub struct SectionListRenderer {
    pub contents: Vec<SectionContent>,
}

#[derive(Debug, Deserialize)]
pub struct SectionContent {
    pub item_section_renderer: ItemSectionRenderer,
}

#[derive(Debug, Deserialize)]
pub struct ItemSectionRenderer {
    pub contents: Vec<ItemContent>,
}

#[derive(Debug, Deserialize)]
pub struct ItemContent {
    pub playlist_video_list_renderer: PlaylistVideoListRenderer,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistVideoListRenderer {
    pub contents: Vec<PlaylistVideoContent>,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistVideoContent {
    pub playlist_video_renderer: PlaylistVideoRenderer,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistVideoRenderer {
    pub video_id: String,
    pub title: Title,
    pub short_byline_text: BylineText,
    pub length_seconds: String,
    pub thumbnail: Thumbnail,
}

#[derive(Debug, Deserialize)]
pub struct Title {
    pub runs: Vec<TextRun>,
}

#[derive(Debug, Deserialize)]
pub struct BylineText {
    pub runs: Vec<TextRun>,
}

#[derive(Debug, Deserialize)]
pub struct TextRun {
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_innertube_client_creation() {
        let client = InnerTubeClient::new();
        assert_eq!(client.client_name, "ANDROID");
        assert_eq!(client.client_version, "20.10.38");
        assert!(client.api_key.is_none());
        assert!(client.visitor_id.is_none());
    }

    #[test]
    fn test_innertube_client_with_client() {
        let client = InnerTubeClient::new()
            .with_client("WEB", "2.20251002.00.00");
        
        assert_eq!(client.client_name, "WEB");
        assert_eq!(client.client_version, "2.20251002.00.00");
    }

    #[test]
    fn test_innertube_client_with_visitor_id() {
        let client = InnerTubeClient::new()
            .with_visitor_id("test_visitor_123");
        
        assert_eq!(client.visitor_id, Some("test_visitor_123".to_string()));
    }

    #[test]
    fn test_innertube_client_chaining() {
        let client = InnerTubeClient::new()
            .with_client("IOS", "20.10.38")
            .with_visitor_id("test_visitor_456");
        
        assert_eq!(client.client_name, "IOS");
        assert_eq!(client.client_version, "20.10.38");
        assert_eq!(client.visitor_id, Some("test_visitor_456".to_string()));
    }

    #[test]
    fn test_innertube_client_switch_client_for_error() {
        let mut client = InnerTubeClient::new();
        let error = RytError::RateLimited;
        
        // Should not panic
        client.switch_client_for_error(&error);
    }

    #[test]
    fn test_format_deserialization() {
        let json = r#"{
            "itag": 22,
            "url": "https://example.com/video.mp4",
            "quality": "hd720",
            "mime_type": "video/mp4",
            "bitrate": 1000000,
            "fps": 30,
            "width": 1280,
            "height": 720
        }"#;
        
        let format: Result<Format, _> = serde_json::from_str(json);
        assert!(format.is_ok());
        
        let format = format.unwrap();
        assert_eq!(format.itag, 22);
        assert_eq!(format.url, "https://example.com/video.mp4");
        assert_eq!(format.quality, "hd720");
        assert_eq!(format.mime_type, "video/mp4");
        assert_eq!(format.bitrate, 1000000);
        assert_eq!(format.fps, Some(30));
        assert_eq!(format.width, Some(1280));
        assert_eq!(format.height, Some(720));
    }

    #[test]
    fn test_text_run_deserialization() {
        let json = r#"{"text": "Test Text"}"#;
        let text_run: Result<TextRun, _> = serde_json::from_str(json);
        assert!(text_run.is_ok());
        
        let text_run = text_run.unwrap();
        assert_eq!(text_run.text, "Test Text");
    }

    #[test]
    fn test_thumbnail_deserialization() {
        let json = r#"{
            "thumbnails": [{
                "url": "https://example.com/thumb.jpg",
                "width": 320,
                "height": 180
            }]
        }"#;
        
        let thumbnail: Result<Thumbnail, _> = serde_json::from_str(json);
        assert!(thumbnail.is_ok());
        
        let thumbnail = thumbnail.unwrap();
        assert_eq!(thumbnail.thumbnails.len(), 1);
        assert_eq!(thumbnail.thumbnails[0].url, "https://example.com/thumb.jpg");
        assert_eq!(thumbnail.thumbnails[0].width, 320);
        assert_eq!(thumbnail.thumbnails[0].height, 180);
    }
}
