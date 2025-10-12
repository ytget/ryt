//! Chunked downloader implementation

use crate::core::progress::Progress;
use crate::error::RytError;
use crate::platform::client::VideoClient;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// Chunked downloader configuration
#[derive(Clone)]
pub struct DownloaderConfig {
    /// Chunk size in bytes
    pub chunk_size: u64,
    /// Maximum retries per chunk
    pub max_retries: u32,
    /// Rate limit in bytes per second
    pub rate_limit_bps: Option<u64>,
    /// Progress callback
    pub progress_callback: Option<Arc<dyn Fn(Progress) + Send + Sync>>,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1024 * 1024, // 1MB
            max_retries: 3,
            rate_limit_bps: None,
            progress_callback: None,
        }
    }
}

/// Chunked downloader
pub struct ChunkedDownloader {
    video_client: Arc<Mutex<VideoClient>>,
    config: DownloaderConfig,
    rate_limiter: Option<Arc<Mutex<RateLimiter>>>,
}

/// Rate limiter for controlling download speed
struct RateLimiter {
    bytes_per_second: u64,
    last_update: std::time::Instant,
    bytes_sent: u64,
}

impl RateLimiter {
    fn new(bytes_per_second: u64) -> Self {
        Self {
            bytes_per_second,
            last_update: std::time::Instant::now(),
            bytes_sent: 0,
        }
    }

    async fn wait_if_needed(&mut self, bytes: u64) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_update);
        
        // Calculate how many bytes we should have sent by now
        let expected_bytes = (elapsed.as_secs_f64() * self.bytes_per_second as f64) as u64;
        
        if self.bytes_sent + bytes > expected_bytes {
            // We're going too fast, need to wait
            let excess_bytes = (self.bytes_sent + bytes) - expected_bytes;
            let wait_time = Duration::from_secs_f64(excess_bytes as f64 / self.bytes_per_second as f64);
            
            if wait_time > Duration::from_millis(1) {
                tokio::time::sleep(wait_time).await;
            }
        }
        
        self.bytes_sent += bytes;
        self.last_update = now;
    }
}

impl ChunkedDownloader {
    /// Create a new chunked downloader
    pub fn new() -> Self {
        Self::with_config(DownloaderConfig::default())
    }

    /// Create a new chunked downloader with configuration
    pub fn with_config(config: DownloaderConfig) -> Self {
        // Create HTTP/1.1-only client for media downloads (matches Go ytdlp line 182)
        let mut http_config = crate::platform::client::HttpClientConfig::default();
        http_config.http1_only = true;  // Force HTTP/1.1 for media downloads
        http_config.client_type = crate::platform::client::ClientType::Chrome;
        let video_client = Arc::new(Mutex::new(VideoClient::with_config(http_config)));

        let rate_limiter = config.rate_limit_bps.map(|bps| {
            Arc::new(Mutex::new(RateLimiter::new(bps)))
        });

        Self {
            video_client,
            config,
            rate_limiter,
        }
    }

    /// Download a file from URL to local path.
    /// Strategy: streaming without Range to avoid 403 on YouTube CDN.
    pub async fn download(&self, url: &str, output_path: &Path) -> Result<(), RytError> {
        use tracing::{info, warn};
        
        info!("Starting download from URL: {}", url);
        // Always use streaming without Range
        let tmp_path = output_path.with_extension("tmp");
        let mut file = File::create(&tmp_path).await?;
        
        match self.download_without_chunking(url, &mut file).await {
            Ok(()) => {
                file.flush().await?;
                drop(file);
                tokio::fs::rename(&tmp_path, output_path).await?;
                info!("Download completed successfully");
                Ok(())
            }
            Err(e) => {
                warn!("Streaming download failed: {}, cleaning up temp file", e);
                let _ = tokio::fs::remove_file(&tmp_path).await;
                Err(e)
            }
        }
    }

    /// Download with resume support
    pub async fn download_with_resume(&self, url: &str, output_path: &Path) -> Result<(), RytError> {
        use tracing::warn;
        // Check if file exists and get its size
        let tmp_path = output_path.with_extension("tmp");
        let existing_size = if tmp_path.exists() {
            tokio::fs::metadata(&tmp_path).await?.len()
        } else {
            0
        };
        
        // Try to get total content length, but if all attempts fail (403), proceed with chunked anyway
        let total_size = match self.get_content_length(url).await {
            Ok(size) => size,
            Err(_e) => {
                warn!("Could not determine content length (all clients failed), proceeding with chunked download");
                0
            }
        };
        
        if total_size > 0 && existing_size >= total_size {
            // File is already complete
            return Ok(());
        }
        
        // Open temp file for appending
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tmp_path)
            .await?;
        
        // Download remaining chunks
        let mut downloaded = existing_size;
        let mut progress = Progress::new(total_size);
        progress.update(downloaded);
        
        while downloaded < total_size || total_size == 0 {
            let start = downloaded;
            let end = if total_size > 0 {
                (start + self.config.chunk_size - 1).min(total_size - 1)
            } else {
                // Unknown size: request bounded chunk
                start + self.config.chunk_size - 1
            };
            
            // Download chunk with retry
            let chunk_data = self.download_chunk_with_retry(url, start, end).await?;
            
            // Write chunk to file
            file.write_all(&chunk_data).await?;
            
            // Update progress
            downloaded += chunk_data.len() as u64;
            progress.update(downloaded);
            
            // Report progress
            if let Some(callback) = &self.config.progress_callback {
                callback(progress.clone());
            }
            
            // Rate limiting
            if let Some(rate_limiter) = &self.rate_limiter {
                let mut limiter = rate_limiter.lock().await;
                limiter.wait_if_needed(chunk_data.len() as u64).await;
            }
            
            // If unknown size and we got less than chunk_size, we're probably done
            if total_size == 0 && (chunk_data.len() as u64) < self.config.chunk_size {
                break;
            }
        }
        
        // Flush and sync file
        file.flush().await?;
        file.sync_all().await?;

        // Finalize: rename temp -> final only if we actually wrote data
        drop(file);
        if (total_size == 0 && downloaded > 0) || (total_size > 0 && downloaded >= total_size) {
            tokio::fs::rename(&tmp_path, output_path).await?;
            return Ok(());
        }

        // Nothing downloaded — clean up and return error
        let _ = tokio::fs::remove_file(&tmp_path).await;
        Err(RytError::Generic("Empty download (0 bytes)".to_string()))
    }

    /// Get content length of the file
    async fn get_content_length(&self, url: &str) -> Result<u64, RytError> {
        use tracing::warn;
        use crate::platform::client::ClientType;
        
        // Try all available client types
        let available_clients = ClientType::all();
        
        for (attempt, client_type) in available_clients.iter().enumerate() {
            // Switch to specific client type
            {
                let mut video_client = self.video_client.lock().await;
                video_client.switch_to_client(*client_type);
            }
            
            let video_client = self.video_client.lock().await;
            
            // Try GET request with Range header (YouTube doesn't support HEAD well)
            // Use simple media request to avoid 403 errors
            let response = video_client.create_simple_media_request(
                reqwest::Method::GET, 
                url
            )
            .header("Range", "bytes=0-1")
            .send()
            .await;
            
            match response {
                Ok(resp) => {
                    if resp.status().is_success() || resp.status() == 206 {
                        return self.parse_content_length_from_response(resp).await;
                    } else {
                        warn!("Failed to get content length with client {:?} (status: {}), trying next client...", 
                              client_type, resp.status());
                    }
                }
                Err(_e) => {
                    warn!("Request failed with client {:?}, trying next client...", client_type);
                }
            }
            
            // Exponential backoff
            if attempt < available_clients.len() - 1 {
                let delay = Duration::from_millis(100 * (1 << (attempt % 3)));
                tokio::time::sleep(delay).await;
            }
        }
        
        // If all clients failed, return error to signal caller to proceed with unknown size
        Err(RytError::Generic("Could not determine content length".to_string()))
    }
    
    /// Parse content length from HTTP response
    async fn parse_content_length_from_response(&self, response: reqwest::Response) -> Result<u64, RytError> {
        if let Some(content_range) = response.headers().get("content-range") {
            if let Ok(range_str) = content_range.to_str() {
                // Parse "bytes 0-1/total" format
                if let Some(slash_pos) = range_str.find('/') {
                    let total_str = &range_str[slash_pos + 1..];
                    if let Ok(total) = total_str.parse::<u64>() {
                        return Ok(total);
                    }
                }
            }
        }
        
        // Last resort: use content-length from response
        if let Some(content_length) = response.headers().get("content-length") {
            if let Ok(length) = content_length.to_str() {
                if let Ok(length) = length.parse::<u64>() {
                    return Ok(length);
                }
            }
        }
        
        // Unknown size
        Ok(0)
    }

    /// Download a single chunk with retry logic
    async fn download_chunk_with_retry(&self, url: &str, start: u64, end: u64) -> Result<Vec<u8>, RytError> {
        use tracing::warn;
        let mut last_error = None;
        
        for attempt in 0..self.config.max_retries {
            match self.download_chunk(url, start, end).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    warn!("Chunk download attempt {} failed for bytes {}-{}: {}", attempt + 1, start, end, e);
                    last_error = Some(e);
                    
                    // Exponential backoff
                    if attempt < self.config.max_retries - 1 {
                        let delay = Duration::from_millis(200 * (1 << attempt));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or(RytError::Generic("Chunk download failed".to_string())))
    }

    /// Download a single chunk
    async fn download_chunk(&self, url: &str, start: u64, end: u64) -> Result<Vec<u8>, RytError> {
        use tracing::{debug, warn};
        let range_header = format!("bytes={}-{}", start, end);
        
        debug!("Acquiring video_client lock for chunk download");
        let video_client = self.video_client.lock().await;
        debug!("Lock acquired, creating request for bytes {}-{}", start, end);
        
        // Use simple media request to avoid 403 errors from YouTube
        let response = video_client.create_simple_media_request(
            reqwest::Method::GET, 
            url
        )
        .header("Range", range_header)
        .send()
        .await?;
        
        // Release lock immediately after sending request
        drop(video_client);
        debug!("Lock released after sending request");
        
        let status = response.status();
        debug!("Response received with status: {} for bytes {}-{}", status, start, end);
        
        if !status.is_success() && status != 206 {
            if status.as_u16() == 403 {
                warn!("403 Forbidden for range request {}-{}", start, end);
                return Err(RytError::RateLimited);
            }
            warn!("Unexpected status code {} for range request {}-{}", status, start, end);
            return Err(RytError::DownloadFailed(
                reqwest::Error::from(response.error_for_status().unwrap_err())
            ));
        }
        
        let data = response.bytes().await?;
        debug!("Downloaded {} bytes for range {}-{}", data.len(), start, end);
        Ok(data.to_vec())
    }

    /// Set progress callback
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(Progress) + Send + Sync + 'static,
    {
        self.config.progress_callback = Some(Arc::new(callback));
        self
    }

    /// Set rate limit
    pub fn with_rate_limit(mut self, bytes_per_second: u64) -> Self {
        self.config.rate_limit_bps = Some(bytes_per_second);
        self.rate_limiter = Some(Arc::new(Mutex::new(RateLimiter::new(bytes_per_second))));
        self
    }

    /// Set chunk size
    pub fn with_chunk_size(mut self, chunk_size: u64) -> Self {
        self.config.chunk_size = chunk_size;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Download without chunking when content length is unknown
    async fn download_without_chunking(&self, url: &str, file: &mut File) -> Result<(), RytError> {
        use tracing::{info, warn, debug};
        use crate::platform::client::ClientType;
        
        info!("Downloading without chunking from: {}", url);
        
        // Try with current client first
        // Use simple media request for googlevideo.com to avoid 403 errors from browser-specific headers
        let video_client = self.video_client.lock().await;
        let response = video_client
            .create_simple_media_request(
                reqwest::Method::GET,
                url,
            )
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    // Success! Continue with this response
                    drop(video_client); // Release lock
                    debug!("Download successful with current client, processing response...");
                    return self.process_successful_response(resp, file).await;
                } else if status.as_u16() == 403 {
                    drop(video_client);
                    warn!("403 Forbidden on streaming GET, falling back to chunked");
                    return Err(RytError::RateLimited);
                } else {
                    warn!("Failed with current client (status: {}), trying other clients...", status);
                }
            }
            Err(e) => {
                warn!("Request failed with current client: {}, trying other clients...", e);
            }
        }
        
        // If current client failed, try other clients
        let available_clients = ClientType::all();
        let mut last_error = None;
        
        for (attempt, client_type) in available_clients.iter().enumerate() {
            // Skip if we already tried this client (it's the current one)
            {
                let current_client = self.video_client.lock().await;
                if current_client.current_client_type() == *client_type {
                    continue;
                }
            }
            
            // Switch to specific client type
            {
                let mut video_client = self.video_client.lock().await;
                video_client.switch_to_client(*client_type);
            }
            
            let video_client = self.video_client.lock().await;
            let response = video_client.create_simple_media_request(
                reqwest::Method::GET, 
                url
            )
            .send()
            .await;
            
            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        // Success! Continue with this response
                        drop(video_client); // Release lock
                        debug!("Download successful with client {:?}, processing response...", client_type);
                        return self.process_successful_response(resp, file).await;
                } else {
                    // If 403, stop header-only switching and propagate upwards to allow URL regeneration
                    if status.as_u16() == 403 {
                        drop(video_client);
                        warn!("403 Forbidden on media GET, requiring URL regeneration");
                        return Err(RytError::RateLimited);
                    }
                    last_error = Some(RytError::DownloadFailed(
                        reqwest::Error::from(resp.error_for_status().unwrap_err())
                    ));
                    warn!("Failed with client {:?} (status: {}), trying next client...",
                          client_type, status);
                }
                }
                Err(e) => {
                    last_error = Some(RytError::DownloadFailed(e));
                    warn!("Request failed with client {:?}, trying next client...", client_type);
                }
            }
            
            // Exponential backoff
            if attempt < available_clients.len() - 1 {
                let delay = Duration::from_millis(200 * (1 << (attempt % 3)));
                tokio::time::sleep(delay).await;
            }
        }
        
        Err(last_error.unwrap_or(RytError::Generic("Download failed with all clients".to_string())))
    }
    
    /// Process successful HTTP response for download
    async fn process_successful_response(&self, response: reqwest::Response, file: &mut File) -> Result<(), RytError> {
        use tracing::{debug, info};
        use futures_util::StreamExt;
        
        let mut stream = response.bytes_stream();
        let mut downloaded = 0u64;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_size = chunk.len();
            
            file.write_all(&chunk).await?;
            downloaded += chunk_size as u64;
            
            debug!("Downloaded {} bytes, total: {}", chunk_size, downloaded);
            
            // Report progress if callback is available
            if let Some(callback) = &self.config.progress_callback {
                let mut progress = Progress::new(0); // Unknown total size
                progress.update(downloaded);
                callback(progress);
            }
            
            // Rate limiting
            if let Some(rate_limiter) = &self.rate_limiter {
                let mut limiter = rate_limiter.lock().await;
                limiter.wait_if_needed(chunk_size as u64).await;
            }
        }
        
        file.flush().await?;
        file.sync_all().await?;
        
        info!("Download completed: {} bytes", downloaded);
        Ok(())
    }
}

impl Default for ChunkedDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_config_default() {
        let config = DownloaderConfig::default();
        assert_eq!(config.chunk_size, 1024 * 1024);
        assert_eq!(config.max_retries, 3);
        assert!(config.rate_limit_bps.is_none());
        assert!(config.progress_callback.is_none());
    }

    #[test]
    fn test_chunked_downloader_creation() {
        let downloader = ChunkedDownloader::new();
        assert_eq!(downloader.config.chunk_size, 1024 * 1024);
        assert_eq!(downloader.config.max_retries, 3);
    }

    #[test]
    fn test_chunked_downloader_with_config() {
        let config = DownloaderConfig {
            chunk_size: 512 * 1024,
            max_retries: 5,
            rate_limit_bps: Some(1024 * 1024),
            progress_callback: None,
        };
        
        let downloader = ChunkedDownloader::with_config(config);
        assert_eq!(downloader.config.chunk_size, 512 * 1024);
        assert_eq!(downloader.config.max_retries, 5);
        assert!(downloader.rate_limiter.is_some());
    }

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(1024 * 1024); // 1MB/s
        assert_eq!(limiter.bytes_per_second, 1024 * 1024);
        assert_eq!(limiter.bytes_sent, 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_wait() {
        let mut limiter = RateLimiter::new(1000); // 1KB/s
        let start = std::time::Instant::now();
        
        // Wait for 1KB
        limiter.wait_if_needed(1000).await;
        
        let elapsed = start.elapsed();
        // Should have waited approximately 1 second
        assert!(elapsed >= Duration::from_millis(900));
        assert!(elapsed <= Duration::from_millis(1100));
    }
}
