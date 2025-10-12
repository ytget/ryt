//! Main downloader implementation

use crate::core::video_info::Format;
use crate::core::{FormatSelector, Progress, QualitySelector, VideoInfo};
use crate::download::ChunkedDownloader;
use crate::error::RytError;
use crate::platform::{InnerTubeClient, PlayerResponse};
use crate::utils::{extract_video_id, to_safe_filename};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Main downloader configuration
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// Format selector
    pub format_selector: Option<FormatSelector>,
    /// Desired file extension
    pub desired_ext: Option<String>,
    /// Output path (file or directory)
    pub output_path: Option<PathBuf>,
    /// Rate limit in bytes per second
    pub rate_limit_bps: Option<u64>,
    /// InnerTube client name
    pub client_name: String,
    /// InnerTube client version
    pub client_version: String,
    /// HTTP timeout
    pub timeout: Duration,
    /// Maximum retries
    pub max_retries: u32,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            format_selector: None,
            desired_ext: None,
            output_path: None,
            rate_limit_bps: None,
            client_name: "ANDROID".to_string(), // ANDROID gives direct URLs without cipher complexity
            client_version: "20.10.38".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

/// Botguard configuration
#[derive(Debug, Clone)]
pub struct BotguardConfig {
    /// Botguard mode
    pub mode: crate::platform::botguard::BotguardMode,
    /// Debug mode
    pub debug: bool,
    /// Token TTL
    pub ttl: Duration,
}

impl Default for BotguardConfig {
    fn default() -> Self {
        Self {
            mode: crate::platform::botguard::BotguardMode::Off,
            debug: false,
            ttl: Duration::from_secs(1800), // 30 minutes
        }
    }
}

/// Main downloader struct
pub struct Downloader {
    options: DownloadOptions,
    botguard: BotguardConfig,
    inner_tube: Arc<Mutex<InnerTubeClient>>,
    downloader: Arc<Mutex<ChunkedDownloader>>,
}

impl Downloader {
    /// Create a new downloader with default options
    pub fn new() -> Self {
        Self {
            options: DownloadOptions::default(),
            botguard: BotguardConfig::default(),
            inner_tube: Arc::new(Mutex::new(InnerTubeClient::new())),
            downloader: Arc::new(Mutex::new(ChunkedDownloader::new())),
        }
    }

    /// Set format selector
    pub fn with_format(mut self, selector: &str, ext: &str) -> Self {
        if let Ok(quality) = QualitySelector::from_str(selector) {
            self.options.format_selector = Some(FormatSelector::new(quality).with_extension(ext));
        }
        self
    }

    /// Set output path
    pub fn with_output_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.options.output_path = Some(path.into());
        self
    }

    /// Set progress callback
    pub fn with_progress(self, _callback: impl Fn(Progress) + Send + Sync + 'static) -> Self {
        // TODO: Implement progress callback in ChunkedDownloader
        self
    }

    /// Set rate limit
    pub fn with_rate_limit(mut self, bytes_per_second: u64) -> Self {
        self.options.rate_limit_bps = Some(bytes_per_second);
        self
    }

    /// Set InnerTube client
    pub fn with_innertube_client(mut self, name: &str, version: &str) -> Self {
        self.options.client_name = name.to_string();
        self.options.client_version = version.to_string();
        self
    }

    /// Set Botguard mode
    pub fn with_botguard(mut self, mode: crate::platform::botguard::BotguardMode) -> Self {
        self.botguard.mode = mode;
        self
    }

    /// Set Botguard debug
    pub fn with_botguard_debug(mut self, debug: bool) -> Self {
        self.botguard.debug = debug;
        self
    }

    /// Set Botguard TTL
    pub fn with_botguard_ttl(mut self, ttl: Duration) -> Self {
        self.botguard.ttl = ttl;
        self
    }

    /// Set HTTP timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.options.max_retries = max_retries;
        self
    }

    /// Resolve video URL and get metadata without downloading
    pub async fn resolve_url(&mut self, video_url: &str) -> Result<(String, VideoInfo), RytError> {
        // Extract video ID
        let video_id = extract_video_id(video_url)?;
        info!("Resolving URL for video ID: {}", video_id);

        // Try to get player response with retry logic for age restrictions
        let mut last_error = None;
        let max_retries = 3;

        for attempt in 0..=max_retries {
            let mut inner_tube = self.inner_tube.lock().await;

            match inner_tube.get_player_response(&video_id).await {
                Ok(player_response) => {
                    // Success, continue with processing
                    drop(inner_tube); // Release lock early
                    let (final_url, video_info) = self
                        .process_player_response(player_response, &video_id)
                        .await?;

                    // Professional: WEB fallback causes c=WEB in URL, breaking ANDROID client context
                    // ANDROID already returns valid itag=18 URLs without 'n' parameter - this is OK
                    // Do NOT switch to WEB just for 'n' parameter - it breaks client consistency
                    // YouTube CDN validates that URL's c= parameter matches the client that obtained it

                    return Ok((final_url, video_info));
                }
                Err(RytError::AgeRestricted) => {
                    warn!(
                        "Age restriction detected on attempt {}, switching client",
                        attempt + 1
                    );
                    // Switch client for age restriction
                    inner_tube.switch_client_for_error(&RytError::AgeRestricted);
                    last_error = Some(RytError::AgeRestricted);

                    // Wait before retry
                    if attempt < max_retries {
                        drop(inner_tube); // Release lock before sleep
                        tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
                    }
                }
                Err(e) => {
                    // Non-retryable error or other error
                    return Err(e);
                }
            }
        }

        // If we get here, all retries failed
        Err(last_error.unwrap_or(RytError::AgeRestricted))
    }

    /// Process player response and extract video info
    async fn process_player_response(
        &mut self,
        player_response: PlayerResponse,
        video_id: &str,
    ) -> Result<(String, VideoInfo), RytError> {
        // Parse formats
        let formats = player_response.parse_formats()?;
        debug!("Found {} formats for video {}", formats.len(), video_id);

        // Debug: print all formats
        // println!("📋 Available formats ({}):", formats.len());
        // for (i, format) in formats.iter().enumerate() {
        //     println!("  {}: itag={}, quality={}, url_len={}, needs_deciphering={}",
        //         i, format.itag, format.quality, format.url.len(), format.needs_deciphering());
        //     if let Some(sig_cipher) = &format.signature_cipher {
        //         println!("    signature_cipher: {}", sig_cipher);
        //     }
        // }

        // Check if we got muxed formats (itag 18, 22, etc.) - these are stable and don't get 403
        let all_itags: Vec<u32> = formats.iter().map(|f| f.itag).collect();
        debug!("All itags from ANDROID: {:?}", all_itags);
        let has_muxed = formats.iter().any(|f| matches!(f.itag, 18 | 22 | 43 | 36));
        debug!("has_muxed={}, will try IOS={}", has_muxed, !has_muxed);

        // If only adaptive formats (itag 299+), try to get muxed from IOS client
        let formats = if !has_muxed {
            debug!("No muxed formats found (only adaptive), trying IOS client for itag 18/22");
            // IOS client often returns muxed formats that ANDROID doesn't provide
            let mut ios_inner_tube = InnerTubeClient::new().with_client("IOS", "19.29.1");

            match ios_inner_tube.get_player_response(video_id).await {
                Ok(ios_response) => match ios_response.parse_formats() {
                    Ok(ios_formats) if !ios_formats.is_empty() => {
                        let has_ios_muxed = ios_formats.iter().any(|f| matches!(f.itag, 18 | 22));
                        if has_ios_muxed {
                            debug!(
                                "✅ IOS client returned {} formats with muxed (itag 18/22)",
                                ios_formats.len()
                            );
                            ios_formats
                        } else {
                            debug!("IOS also has no muxed formats, using original ANDROID");
                            formats
                        }
                    }
                    _ => {
                        debug!("IOS response parse failed or empty, using ANDROID");
                        formats
                    }
                },
                Err(e) => {
                    warn!("IOS client request failed: {}, using ANDROID formats", e);
                    formats
                }
            }
        } else {
            formats
        };

        // Strongly prefer muxed formats (itag 18/22) to avoid 403
        let selected_format = formats
            .iter()
            .filter(|f| matches!(f.itag, 18 | 22))
            .max_by_key(|f| f.height.unwrap_or(0))
            .or_else(|| {
                formats
                    .iter()
                    .filter(|f| matches!(f.itag, 43 | 36))
                    .max_by_key(|f| f.height.unwrap_or(0))
            })
            .or_else(|| self.select_format(&formats).ok())
            .ok_or_else(|| RytError::NoFormatFound)?;
        debug!(
            "Selected format: itag={}, quality={}, size={} (muxed={})",
            selected_format.itag,
            selected_format.quality,
            selected_format.size.unwrap_or(0),
            matches!(selected_format.itag, 18 | 22 | 43 | 36)
        );

        // Resolve final URL with signature deciphering
        let mut final_url = if selected_format.needs_deciphering() {
            debug!("Format requires deciphering, resolving cipher...");
            let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
            self.resolve_format_url_with_cipher(selected_format, &video_url)
                .await?
        } else {
            debug!("Format does not require deciphering");
            selected_format.url.clone()
        };

        // Normalize final_url for direct URL path as well (ratebypass, alr, n, drop rqh)
        if let Ok(mut parsed) = url::Url::parse(&final_url) {
            // If n present, try to decode and rewrite query pairs safely
            if let Some(n_val) = parsed
                .query_pairs()
                .find(|(k, _)| k == "n")
                .map(|(_, v)| v.to_string())
            {
                let cipher = crate::platform::cipher::Cipher::new();
                if let Ok(n_out) = cipher
                    .decipher_n_parameter(
                        &n_val,
                        &format!("https://www.youtube.com/watch?v={}", video_id),
                    )
                    .await
                {
                    // Collect pairs first (immutable borrow), then rebuild (mutable borrow)
                    let pairs: Vec<(String, String)> = parsed
                        .query_pairs()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect();
                    let mut new_pairs: Vec<(String, String)> = Vec::with_capacity(pairs.len() + 1);
                    let mut replaced = false;
                    for (k, v) in pairs {
                        if k == "n" {
                            new_pairs.push((k, n_out.clone()));
                            replaced = true;
                        } else {
                            new_pairs.push((k, v));
                        }
                    }
                    if !replaced {
                        new_pairs.push(("n".to_string(), n_out));
                    }
                    {
                        let mut qp = parsed.query_pairs_mut();
                        qp.clear()
                            .extend_pairs(new_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                    }
                }
            }
            // Add missing critical params (DO NOT remove rqh!)
            let sparams_val = parsed
                .query_pairs()
                .find(|(k, _)| k == "sparams")
                .map(|(_, v)| v.to_string());
            let has_rqh = parsed.query_pairs().any(|(k, _)| k == "rqh");
            let sparams_has_rqh = sparams_val.as_ref().map_or(false, |s| s.contains("rqh"));
            let has_ratebypass = parsed.query_pairs().any(|(k, _)| k == "ratebypass");
            let has_alr = parsed.query_pairs().any(|(k, _)| k == "alr");

            debug!(
                "Direct URL norm: has_rqh={}, sparams_has_rqh={}, adding={}",
                has_rqh,
                sparams_has_rqh,
                sparams_has_rqh && !has_rqh
            );

            {
                let mut qp = parsed.query_pairs_mut();
                if !has_ratebypass {
                    qp.append_pair("ratebypass", "yes");
                }
                if !has_alr {
                    qp.append_pair("alr", "yes");
                }
                // CRITICAL FIX: Add rqh=1 if sparams lists it (required for itag=18)
                if sparams_has_rqh && !has_rqh {
                    qp.append_pair("rqh", "1");
                    debug!(
                        "✅ CRITICAL: Added rqh=1 to direct URL (itag={})",
                        selected_format.itag
                    );
                }
            }
            let s: String = parsed.into();
            final_url = s;
        }

        // println!("🔗 Selected format: itag={}, quality={}, url={}", selected_format.itag, selected_format.quality, final_url);
        // if let Some(sig_cipher) = &selected_format.signature_cipher {
        //     println!("🔐 Signature cipher: {}", sig_cipher);
        // }

        // Create video info
        let video_info = VideoInfo {
            id: video_id.to_string(),
            title: player_response
                .video_details
                .as_ref()
                .map(|v| v.title.clone())
                .unwrap_or_default(),
            author: player_response
                .video_details
                .as_ref()
                .map(|v| v.author.clone())
                .unwrap_or_default(),
            duration: player_response
                .video_details
                .as_ref()
                .and_then(|v| v.length_seconds.parse().ok())
                .unwrap_or(0),
            description: player_response
                .video_details
                .as_ref()
                .map(|v| v.short_description.clone())
                .unwrap_or_default(),
            formats,
            thumbnail: player_response
                .video_details
                .as_ref()
                .and_then(|v| v.thumbnail.thumbnails.first())
                .map(|t| t.url.clone()),
            upload_date: None,
            view_count: None,
            like_count: None,
            tags: Vec::new(),
            category: None,
        };

        Ok((final_url, video_info))
    }

    /// Download video to file
    pub async fn download(&mut self, video_url: &str) -> Result<VideoInfo, RytError> {
        // Resolve URL and get metadata (first attempt)
        let (mut final_url, mut video_info) = self.resolve_url(video_url).await?;
        info!("Starting download for: {}", video_info.title);

        // Determine output path
        let output_path = self.determine_output_path(&video_info)?;
        debug!("Output path: {:?}", output_path);

        // Try download with limited retries; on 403/RateLimited regenerate URL and retry
        let max_attempts = 2u32;
        for attempt in 1..=max_attempts {
            let downloader = self.downloader.lock().await;
            let result = downloader.download(&final_url, &output_path).await;
            drop(downloader);

            match result {
                Ok(()) => {
                    info!("Download completed successfully");
                    // Update video info with output path
                    video_info.title = output_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("video")
                        .to_string();
                    return Ok(video_info);
                }
                Err(RytError::RateLimited) if attempt < max_attempts => {
                    warn!("Rate limited/403 during media download (attempt {}/{}). Regenerating URL and retrying...", attempt, max_attempts);
                    // Switch client strategy for error and regenerate URL
                    {
                        let mut inner = self.inner_tube.lock().await;
                        inner.switch_client_for_error(&RytError::RateLimited);
                    }
                    // Resolve again to get fresh final_url
                    let (new_url, _vi) = self.resolve_url(video_url).await?;
                    final_url = new_url;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        // If we got here, all attempts failed
        Err(RytError::Generic(
            "Download failed after retries".to_string(),
        ))
    }

    /// Download playlist
    pub async fn download_playlist(
        &mut self,
        playlist_url: &str,
        limit: Option<usize>,
    ) -> Result<Vec<VideoInfo>, RytError> {
        // Extract playlist ID
        let playlist_id = crate::utils::url::extract_playlist_id(playlist_url)?;

        // Get playlist items
        let items = {
            let mut inner_tube = self.inner_tube.lock().await;
            inner_tube.get_playlist_items(&playlist_id, limit).await?
        };

        // Download each video
        let mut results = Vec::new();
        for item in items {
            let video_url = format!("https://www.youtube.com/watch?v={}", item.video_id);
            match self.download(&video_url).await {
                Ok(info) => results.push(info),
                Err(e) => {
                    eprintln!("Failed to download {}: {}", item.title, e);
                    continue;
                }
            }
        }

        Ok(results)
    }

    /// Select format based on selector
    fn select_format<'a>(&self, formats: &'a [Format]) -> Result<&'a Format, RytError> {
        let default_selector = FormatSelector::new(QualitySelector::Best);
        let selector = self
            .options
            .format_selector
            .as_ref()
            .unwrap_or(&default_selector);

        let mut candidates: Vec<&Format> = formats.iter().collect();

        // Filter by extension
        if let Some(ext) = &selector.extension {
            candidates.retain(|f| f.mime_type.contains(ext));
        }

        // Filter by height constraints
        if let Some(height_limit) = selector.height_limit {
            candidates.retain(|f| {
                let height = f.height.unwrap_or(0);
                height <= height_limit
            });
        }

        if let Some(height_min) = selector.height_min {
            candidates.retain(|f| {
                let height = f.height.unwrap_or(0);
                height >= height_min
            });
        }

        // Select by quality
        match &selector.quality {
            QualitySelector::Best => {
                candidates.sort_by(|a, b| b.bitrate.cmp(&a.bitrate));
                candidates.first().copied()
            }
            QualitySelector::Worst => {
                candidates.sort_by(|a, b| a.bitrate.cmp(&b.bitrate));
                candidates.first().copied()
            }
            QualitySelector::Itag(target_itag) => {
                candidates.iter().find(|f| f.itag == *target_itag).copied()
            }
            QualitySelector::Height(target_height) => candidates
                .iter()
                .filter(|f| f.height.unwrap_or(0) == *target_height)
                .max_by_key(|f| f.bitrate)
                .copied(),
            QualitySelector::HeightLessOrEqual(target_height) => candidates
                .iter()
                .filter(|f| f.height.unwrap_or(0) <= *target_height)
                .max_by_key(|f| f.bitrate)
                .copied(),
            QualitySelector::HeightGreaterOrEqual(target_height) => candidates
                .iter()
                .filter(|f| f.height.unwrap_or(0) >= *target_height)
                .max_by_key(|f| f.bitrate)
                .copied(),
        }
        .ok_or(RytError::NoFormatFound)
    }

    /// Resolve format URL with signature deciphering
    async fn resolve_format_url_with_cipher(
        &self,
        format: &Format,
        video_url: &str,
    ) -> Result<String, RytError> {
        use crate::platform::cipher::Cipher;

        // println!("🔧 Starting cipher resolution for format itag={}", format.itag);
        let cipher = Cipher::new();
        let mut final_url = format.url.clone();

        // Handle signature cipher
        if let Some(sig_cipher) = &format.signature_cipher {
            // println!("🔧 Parsing signature cipher: {}", sig_cipher);

            // Parse signature cipher parameters
            let sig_params: std::collections::HashMap<String, String> =
                url::form_urlencoded::parse(sig_cipher.as_bytes())
                    .into_owned()
                    .collect();

            // println!("🔧 Parsed parameters: {:?}", sig_params);

            if let Some(base_url) = sig_params.get("url") {
                final_url = base_url.clone();
                // println!("🔧 Base URL: {}", final_url);
            }

            if let Some(signature) = sig_params.get("s") {
                println!("🔧 Deciphering signature: {}", signature);
                let deciphered_sig = cipher.decipher_signature(signature, video_url).await?;
                println!("🔧 Deciphered signature: {}", deciphered_sig);

                // Replace existing sig parameter or add new one
                let sig_regex = regex::Regex::new(r"[?&]sig=([^&]+)")?;
                if sig_regex.is_match(&final_url) {
                    // Replace existing sig parameter
                    final_url = sig_regex
                        .replace(&final_url, |caps: &regex::Captures| {
                            format!(
                                "{}sig={}",
                                &caps[0][..caps[0].find('=').unwrap() + 1],
                                deciphered_sig
                            )
                        })
                        .to_string();
                    println!("🔧 Replaced sig parameter in URL");
                } else {
                    // Add new sig parameter
                    final_url = format!("{}&sig={}", final_url, deciphered_sig);
                    println!("🔧 Added sig parameter to URL");
                }
                println!("🔧 Final URL with deciphered sig: {}", final_url);
            }

            if let Some(n_param) = sig_params.get("n") {
                // println!("🔧 Deciphering n-parameter: {}", n_param);
                let deciphered_n = cipher.decipher_n_parameter(n_param, video_url).await?;
                // println!("🔧 Deciphered n-parameter: {}", deciphered_n);

                // Add n-parameter to URL
                if final_url.contains("&n=") {
                    final_url = final_url.replace("&n=", &format!("&n={}", deciphered_n));
                } else {
                    final_url = format!("{}&n={}", final_url, deciphered_n);
                }
            }
        }

        // Handle n-parameter in URL
        if final_url.contains("&n=") || final_url.contains("?n=") {
            let n_regex = regex::Regex::new(r"[?&]n=([^&]+)")?;
            if let Some(captures) = n_regex.captures(&final_url) {
                if let Some(n_param) = captures.get(1) {
                    let deciphered_n = cipher
                        .decipher_n_parameter(n_param.as_str(), video_url)
                        .await?;
                    final_url = final_url.replace(n_param.as_str(), &deciphered_n);
                }
            }
        }

        // Normalize URL parameters similar to Go ytdlp:
        // - ensure n is decoded if present (handled above already)
        // - enforce ratebypass=yes
        // - add alr=yes to encourage stable redirects
        if let Ok(mut parsed) = url::Url::parse(&final_url) {
            // Precompute existence flags before taking a mutable borrow
            let has_ratebypass = parsed.query_pairs().any(|(k, _)| k == "ratebypass");
            let has_alr = parsed.query_pairs().any(|(k, _)| k == "alr");

            // Add missing params. If sparams contains rqh but rqh parameter is missing, add it
            let sparams_val = parsed
                .query_pairs()
                .find(|(k, _)| k == "sparams")
                .map(|(_, v)| v.to_string());
            let has_rqh = parsed.query_pairs().any(|(k, _)| k == "rqh");
            let sparams_has_rqh = sparams_val.as_ref().map_or(false, |s| s.contains("rqh"));
            let current_fvip = parsed
                .query_pairs()
                .find(|(k, _)| k == "fvip")
                .map(|(_, v)| v.to_string());

            debug!(
                "URL normalization: has_rqh={}, sparams_has_rqh={}, will add rqh={}",
                has_rqh,
                sparams_has_rqh,
                sparams_has_rqh && !has_rqh
            );

            {
                let mut qp = parsed.query_pairs_mut();
                if !has_ratebypass {
                    qp.append_pair("ratebypass", "yes");
                    debug!("Added ratebypass=yes");
                }
                if !has_alr {
                    qp.append_pair("alr", "yes");
                    debug!("Added alr=yes");
                }
                // If sparams lists rqh but rqh param is missing, add rqh=1 (CRITICAL for itag 18)
                if sparams_has_rqh && !has_rqh {
                    qp.append_pair("rqh", "1");
                    debug!("✅ Added rqh=1 (required by sparams)");
                }
            }

            // Professional 403 mitigation: rotate fvip (front video IP) to get different CDN server
            // YouTube uses fvip=1..5 for load balancing; rotating helps bypass temporary blocks
            if let Some(fvip_str) = current_fvip {
                if let Ok(fvip_num) = fvip_str.parse::<u8>() {
                    // Cycle through fvip 1-5
                    let new_fvip = (fvip_num % 5) + 1;
                    // Rebuild query without fvip
                    let pairs: Vec<(String, String)> = parsed
                        .query_pairs()
                        .filter(|(k, _)| k != "fvip")
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect();
                    {
                        let mut qp = parsed.query_pairs_mut();
                        qp.clear()
                            .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                        qp.append_pair("fvip", &new_fvip.to_string());
                    }
                    debug!(
                        "Rotated fvip from {} to {} for CDN failover",
                        fvip_num, new_fvip
                    );
                }
            }

            let s: String = parsed.into();
            return Ok(s);
        }

        Ok(final_url)
    }

    /// Determine output path for downloaded file
    fn determine_output_path(&self, video_info: &VideoInfo) -> Result<PathBuf, RytError> {
        if let Some(output_path) = &self.options.output_path {
            if output_path.is_dir() {
                // Generate filename from title
                let ext = self.options.desired_ext.as_deref().unwrap_or("mp4");
                let safe_filename = to_safe_filename(&video_info.title, ext);
                Ok(output_path.join(safe_filename))
            } else {
                // Use provided path as-is
                Ok(output_path.clone())
            }
        } else {
            // Generate filename in current directory
            let ext = self.options.desired_ext.as_deref().unwrap_or("mp4");
            let safe_filename = to_safe_filename(&video_info.title, ext);
            Ok(PathBuf::from(safe_filename))
        }
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_creation() {
        let downloader = Downloader::new();
        assert_eq!(downloader.options.client_name, "ANDROID");
        assert_eq!(downloader.options.client_version, "20.10.38");
    }

    #[test]
    fn test_downloader_with_format() {
        let downloader = Downloader::new()
            .with_format("best", "mp4")
            .with_rate_limit(1024 * 1024); // 1MB/s

        assert!(downloader.options.format_selector.is_some());
        assert_eq!(downloader.options.rate_limit_bps, Some(1024 * 1024));
    }

    #[test]
    fn test_downloader_with_botguard() {
        let downloader = Downloader::new()
            .with_botguard(crate::platform::botguard::BotguardMode::Auto)
            .with_botguard_debug(true);

        assert_eq!(
            downloader.botguard.mode,
            crate::platform::botguard::BotguardMode::Auto
        );
        assert!(downloader.botguard.debug);
    }
}
