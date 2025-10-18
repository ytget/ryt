//! HTTP client for video platform API requests

use crate::error::RytError;
use reqwest::{Client, ClientBuilder};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Client types for realistic header emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientType {
    Chrome,
    Firefox,
    Safari,
    Android,
    Ios,
    Edge,
    Opera,
    SamsungBrowser,
    AndroidTV,
    SmartTV,
}

impl ClientType {
    /// Get all available client types
    pub fn all() -> Vec<ClientType> {
        vec![
            ClientType::Chrome,
            ClientType::Firefox,
            ClientType::Safari,
            ClientType::Android,
            ClientType::Ios,
            ClientType::Edge,
            ClientType::Opera,
            ClientType::SamsungBrowser,
            ClientType::AndroidTV,
            ClientType::SmartTV,
        ]
    }

    /// Get client type from string
    pub fn from_str(s: &str) -> Option<ClientType> {
        match s.to_lowercase().as_str() {
            "chrome" => Some(ClientType::Chrome),
            "firefox" => Some(ClientType::Firefox),
            "safari" => Some(ClientType::Safari),
            "android" => Some(ClientType::Android),
            "ios" => Some(ClientType::Ios),
            "edge" => Some(ClientType::Edge),
            "opera" => Some(ClientType::Opera),
            "samsung" => Some(ClientType::SamsungBrowser),
            "androidtv" => Some(ClientType::AndroidTV),
            "smarttv" => Some(ClientType::SmartTV),
            _ => None,
        }
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        match self {
            ClientType::Chrome => "chrome".to_string(),
            ClientType::Firefox => "firefox".to_string(),
            ClientType::Safari => "safari".to_string(),
            ClientType::Android => "android".to_string(),
            ClientType::Ios => "ios".to_string(),
            ClientType::Edge => "edge".to_string(),
            ClientType::Opera => "opera".to_string(),
            ClientType::SamsungBrowser => "samsung".to_string(),
            ClientType::AndroidTV => "androidtv".to_string(),
            ClientType::SmartTV => "smarttv".to_string(),
        }
    }

    /// Check if this is a mobile client
    pub fn is_mobile(&self) -> bool {
        matches!(
            self,
            ClientType::Android | ClientType::Ios | ClientType::SamsungBrowser
        )
    }

    /// Check if this is a web client
    pub fn is_web(&self) -> bool {
        matches!(
            self,
            ClientType::Chrome
                | ClientType::Firefox
                | ClientType::Safari
                | ClientType::Edge
                | ClientType::Opera
        )
    }

    /// Check if this is a TV client
    pub fn is_tv(&self) -> bool {
        matches!(self, ClientType::AndroidTV | ClientType::SmartTV)
    }
}

/// HTTP client configuration
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Request timeout
    pub timeout: Duration,
    /// Maximum retries
    pub max_retries: u32,
    /// User agent string
    pub user_agent: Option<String>,
    /// Proxy URL
    pub proxy_url: Option<String>,
    /// Current client type
    pub client_type: ClientType,
    /// Enable client switching
    pub enable_client_switching: bool,
    /// Client switching strategy
    pub switching_strategy: ClientSwitchingStrategy,
    /// Force HTTP/1.1 only (disable HTTP/2)
    pub http1_only: bool,
}

/// Client switching strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSwitchingStrategy {
    /// Round-robin switching
    RoundRobin,
    /// Random switching
    Random,
    /// Switch on error (403, rate limit)
    OnError,
    /// Switch on geographic restrictions
    OnGeoBlock,
    /// Smart switching based on response
    Smart,
}

impl Default for ClientSwitchingStrategy {
    fn default() -> Self {
        Self::Smart
    }
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            user_agent: None,
            proxy_url: None,
            client_type: ClientType::Chrome,
            enable_client_switching: true,
            switching_strategy: ClientSwitchingStrategy::default(),
            http1_only: false, // HTTP/2 by default
        }
    }
}

/// YouTube HTTP client
pub struct VideoClient {
    client: Client,
    config: HttpClientConfig,
    current_client_index: usize,
    client_switch_count: u32,
}

impl VideoClient {
    /// Create a new YouTube client with default configuration
    pub fn new() -> Self {
        Self::with_config(HttpClientConfig::default())
    }

    /// Create a new YouTube client with custom configuration
    pub fn with_config(config: HttpClientConfig) -> Self {
        let mut builder = ClientBuilder::new()
            .timeout(config.timeout)
            .gzip(true)
            .brotli(true);

        // Force HTTP/1.1 if requested (for media downloads, matches Go ytdlp)
        if config.http1_only {
            builder = builder.http1_only();
        }

        // Set user agent
        if let Some(user_agent) = &config.user_agent {
            builder = builder.user_agent(user_agent);
        } else {
            // Default Android user agent
            builder = builder
                .user_agent("com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip");
        }

        // Set proxy
        if let Some(proxy_url) = &config.proxy_url {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }

        let client = builder.build().expect("Failed to build HTTP client");

        Self {
            client,
            config,
            current_client_index: 0,
            client_switch_count: 0,
        }
    }

    /// Get the underlying HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get current client type
    pub fn current_client_type(&self) -> ClientType {
        self.config.client_type
    }

    /// Switch to next client type
    pub fn switch_client(&mut self) -> ClientType {
        if !self.config.enable_client_switching {
            return self.config.client_type;
        }

        let available_clients = ClientType::all();
        self.current_client_index = (self.current_client_index + 1) % available_clients.len();
        self.client_switch_count += 1;

        let new_client_type = available_clients[self.current_client_index];
        self.config.client_type = new_client_type;

        info!(
            "Switched to client type: {:?} (switch #{}",
            new_client_type, self.client_switch_count
        );
        new_client_type
    }

    /// Switch to specific client type
    pub fn switch_to_client(&mut self, client_type: ClientType) {
        self.config.client_type = client_type;
        self.client_switch_count += 1;

        // Update index
        let available_clients = ClientType::all();
        if let Some(index) = available_clients.iter().position(|&c| c == client_type) {
            self.current_client_index = index;
        }

        info!(
            "Switched to specific client type: {:?} (switch #{})",
            client_type, self.client_switch_count
        );
    }

    /// Switch client based on strategy
    pub fn switch_client_by_strategy(&mut self, error: Option<&RytError>) -> ClientType {
        match self.config.switching_strategy {
            ClientSwitchingStrategy::RoundRobin => self.switch_client(),
            ClientSwitchingStrategy::Random => {
                use rand::Rng;
                let available_clients = ClientType::all();
                let random_index = rand::thread_rng().gen_range(0..available_clients.len());
                let new_client_type = available_clients[random_index];
                self.switch_to_client(new_client_type);
                new_client_type
            }
            ClientSwitchingStrategy::OnError => {
                if let Some(err) = error {
                    match err {
                        RytError::RateLimited | RytError::BotguardError(_) => {
                            // Switch to mobile client for better success rate
                            if self.config.client_type.is_web() {
                                self.switch_to_client(ClientType::Android);
                                ClientType::Android
                            } else {
                                self.switch_client()
                            }
                        }
                        RytError::AgeRestricted => {
                            // Switch to mobile client to bypass age restrictions
                            if self.config.client_type.is_web() {
                                self.switch_to_client(ClientType::Android);
                                ClientType::Android
                            } else {
                                self.switch_client()
                            }
                        }
                        _ => self.config.client_type,
                    }
                } else {
                    self.config.client_type
                }
            }
            ClientSwitchingStrategy::OnGeoBlock => {
                if let Some(err) = error {
                    match err {
                        RytError::VideoUnavailable => {
                            // Switch to mobile client to bypass geo-blocking
                            if self.config.client_type.is_web() {
                                self.switch_to_client(ClientType::Android);
                                ClientType::Android
                            } else {
                                self.switch_client()
                            }
                        }
                        _ => self.config.client_type,
                    }
                } else {
                    self.config.client_type
                }
            }
            ClientSwitchingStrategy::Smart => {
                // Smart switching: combine multiple strategies
                if let Some(err) = error {
                    match err {
                        RytError::RateLimited | RytError::BotguardError(_) => {
                            // Always switch to next client for rate limiting
                            self.switch_client()
                        }
                        RytError::VideoUnavailable => {
                            // Always switch to next client for video unavailable
                            self.switch_client()
                        }
                        RytError::AgeRestricted => {
                            // Switch to mobile client to bypass age restrictions
                            if self.config.client_type.is_web() {
                                self.switch_to_client(ClientType::Android);
                                ClientType::Android
                            } else {
                                self.switch_client()
                            }
                        }
                        _ => self.switch_client(),
                    }
                } else {
                    // No error, use round-robin
                    self.switch_client()
                }
            }
        }
    }

    /// Get client switch count
    pub fn client_switch_count(&self) -> u32 {
        self.client_switch_count
    }

    /// Reset client switching
    pub fn reset_client_switching(&mut self) {
        self.current_client_index = 0;
        self.client_switch_count = 0;
        self.config.client_type = ClientType::Chrome;
    }

    /// Get client configuration
    pub fn config(&self) -> &HttpClientConfig {
        &self.config
    }

    /// Create a request with common YouTube headers
    pub fn create_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .header("Accept", "*/*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive")
            .header("Cache-Control", "no-cache")
            .header("DNT", "1")
            .header("Upgrade-Insecure-Requests", "1")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Site", "none")
            .header("Sec-Fetch-User", "?1")
    }

    /// Create a request with realistic browser headers using current client type
    pub fn create_realistic_request(
        &self,
        method: reqwest::Method,
        url: &str,
    ) -> reqwest::RequestBuilder {
        self.create_realistic_request_with_client(method, url, self.config.client_type)
    }

    /// Create a simple request for media downloads (googlevideo.com) without browser-specific headers
    pub fn create_simple_media_request(
        &self,
        method: reqwest::Method,
        url: &str,
    ) -> reqwest::RequestBuilder {
        // Use minimal headers for media downloads to avoid 403 errors
        // Match Go ytdlp exactly: User-Agent, Accept, Accept-Encoding, Connection, Cache-Control
        self.client
            .request(method, url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36")
            .header("Accept", "*/*")
            .header("Accept-Encoding", "identity")
            .header("Connection", "keep-alive")
            .header("Cache-Control", "no-cache")
    }

    /// Create a request with realistic browser headers for specific client type
    pub fn create_realistic_request_with_client(
        &self,
        method: reqwest::Method,
        url: &str,
        client_type: ClientType,
    ) -> reqwest::RequestBuilder {
        let (user_agent, headers) = match client_type {
            ClientType::Chrome => (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                vec![
                    ("Sec-Ch-Ua", r#""Not_A Brand";v="8", "Chromium";v="120", "Google Chrome";v="120""#),
                    ("Sec-Ch-Ua-Mobile", "?0"),
                    ("Sec-Ch-Ua-Platform", r#""Windows""#),
                ]
            ),
            ClientType::Firefox => (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
                vec![
                    ("Sec-Fetch-Dest", "document"),
                    ("Sec-Fetch-Mode", "navigate"),
                    ("Sec-Fetch-Site", "none"),
                    ("Sec-Fetch-User", "?1"),
                ]
            ),
            ClientType::Safari => (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15",
                vec![
                    ("Sec-Fetch-Dest", "document"),
                    ("Sec-Fetch-Mode", "navigate"),
                    ("Sec-Fetch-Site", "none"),
                    ("Sec-Fetch-User", "?1"),
                ]
            ),
            ClientType::Android => (
                "Mozilla/5.0 (Linux; Android 11; SM-G973F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36",
                vec![
                    ("Sec-Ch-Ua-Mobile", "?1"),
                    ("Sec-Ch-Ua-Platform", r#""Android""#),
                ]
            ),
            ClientType::Ios => (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Mobile/15E148 Safari/604.1",
                vec![
                    ("Sec-Ch-Ua-Mobile", "?1"),
                    ("Sec-Ch-Ua-Platform", r#""iOS""#),
                ]
            ),
            ClientType::Edge => (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
                vec![
                    ("Sec-Ch-Ua", r#""Not_A Brand";v="8", "Chromium";v="120", "Microsoft Edge";v="120""#),
                    ("Sec-Ch-Ua-Mobile", "?0"),
                    ("Sec-Ch-Ua-Platform", r#""Windows""#),
                ]
            ),
            ClientType::Opera => (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 OPR/106.0.0.0",
                vec![
                    ("Sec-Ch-Ua", r#""Not_A Brand";v="8", "Chromium";v="120", "Opera";v="106""#),
                    ("Sec-Ch-Ua-Mobile", "?0"),
                    ("Sec-Ch-Ua-Platform", r#""Windows""#),
                ]
            ),
            ClientType::SamsungBrowser => (
                "Mozilla/5.0 (Linux; Android 12; SM-G998B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/22.0 Chrome/120.0.0.0 Mobile Safari/537.36",
                vec![
                    ("Sec-Ch-Ua-Mobile", "?1"),
                    ("Sec-Ch-Ua-Platform", r#""Android""#),
                ]
            ),
            ClientType::AndroidTV => (
                "Mozilla/5.0 (Linux; Android 11; ADT-3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                vec![
                    ("Sec-Ch-Ua-Mobile", "?0"),
                    ("Sec-Ch-Ua-Platform", r#""Android""#),
                ]
            ),
            ClientType::SmartTV => (
                "Mozilla/5.0 (Web0S; Linux/SmartTV) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                vec![
                    ("Sec-Ch-Ua-Mobile", "?0"),
                    ("Sec-Ch-Ua-Platform", r#""Linux""#),
                ]
            ),
        };

        let mut request = self.client
            .request(method, url)
            .header("User-Agent", user_agent)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive")
            .header("Cache-Control", "no-cache")
            .header("DNT", "1")
            .header("Upgrade-Insecure-Requests", "1");

        // Add client-specific headers
        for (name, value) in headers {
            request = request.header(name, value);
        }

        request
    }

    /// Create a request for InnerTube API with client-specific headers
    pub fn create_innertube_request(&self, url: &str) -> reqwest::RequestBuilder {
        // Add INNERTUBE_API_KEY to URL based on client type
        let (api_key, client_name, client_version) = match self.config.client_type {
            ClientType::Chrome
            | ClientType::Firefox
            | ClientType::Safari
            | ClientType::Edge
            | ClientType::Opera => (
                "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8",
                "1",
                "2.20251002.00.00",
            ),
            ClientType::Android | ClientType::SamsungBrowser | ClientType::AndroidTV => {
                ("AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w", "3", "20.10.38")
            }
            ClientType::Ios => ("AIzaSyBUPetSUmoZL-OhlxA7wSac5XinrygCqMo", "5", "20.10.38"),
            ClientType::SmartTV => (
                "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8",
                "1",
                "2.20251002.00.00",
            ),
        };

        let url_with_key = if url.contains('?') {
            format!("{}&key={}", url, api_key)
        } else {
            format!("{}?key={}", url, api_key)
        };

        let mut request = self
            .create_request(reqwest::Method::POST, &url_with_key)
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", client_name)
            .header("X-YouTube-Client-Version", client_version);

        // Add device-specific headers based on client type
        match self.config.client_type {
            ClientType::Android => {
                request = request
                    .header("X-YouTube-Device-Model", "SM-G973F")
                    .header("X-YouTube-Device-Os", "Android")
                    .header("X-YouTube-Device-Os-Version", "11")
                    .header("X-YouTube-Device-Connection-Type", "WIFI")
                    .header("X-YouTube-Device-Memory-Mb", "4096")
                    .header("X-YouTube-Device-Display-Density", "420")
                    .header("X-YouTube-Device-Display-Height", "2280")
                    .header("X-YouTube-Device-Display-Width", "1080")
                    .header("X-YouTube-Device-Cpu-Cores", "8")
                    .header("X-YouTube-Device-Cpu-Model", "Exynos 9820")
                    .header("X-YouTube-Device-Gpu-Model", "Mali-G76 MP12")
                    .header("X-YouTube-Device-Gpu-Version", "OpenGL ES 3.2")
                    .header("X-YouTube-Device-Screen-Density", "420")
                    .header("X-YouTube-Device-Screen-Height", "2280")
                    .header("X-YouTube-Device-Screen-Width", "1080")
                    .header("X-YouTube-Device-Timezone", "UTC")
                    .header("X-YouTube-Device-Language", "en")
                    .header("X-YouTube-Device-Country", "US")
                    .header("X-YouTube-Device-Region", "US")
                    .header("X-YouTube-Device-Carrier", "Unknown")
                    .header("X-YouTube-Device-Network-Type", "WIFI")
                    .header("X-YouTube-Device-Network-Speed", "1000000")
                    .header("X-YouTube-Device-Network-Signal", "4")
                    .header("X-YouTube-Device-Battery-Level", "100")
                    .header("X-YouTube-Device-Battery-Charging", "false")
                    .header("X-YouTube-Device-Volume", "100")
                    .header("X-YouTube-Device-Brightness", "100")
                    .header("X-YouTube-Device-Orientation", "portrait")
                    .header("X-YouTube-Device-Accelerometer", "true")
                    .header("X-YouTube-Device-Gyroscope", "true")
                    .header("X-YouTube-Device-Magnetometer", "true")
                    .header("X-YouTube-Device-Proximity", "false")
                    .header("X-YouTube-Device-Light", "true")
                    .header("X-YouTube-Device-Pressure", "false")
                    .header("X-YouTube-Device-Temperature", "false")
                    .header("X-YouTube-Device-Humidity", "false")
                    .header("X-YouTube-Device-Altitude", "0")
                    .header("X-YouTube-Device-Latitude", "0")
                    .header("X-YouTube-Device-Longitude", "0")
                    .header("X-YouTube-Device-Accuracy", "0")
                    .header("X-YouTube-Device-Speed", "0")
                    .header("X-YouTube-Device-Heading", "0")
                    .header("X-YouTube-Device-Timestamp", "0");
            }
            ClientType::Ios => {
                request = request
                    .header("X-YouTube-Device-Model", "iPhone13,2")
                    .header("X-YouTube-Device-Os", "iOS")
                    .header("X-YouTube-Device-Os-Version", "17.1")
                    .header("X-YouTube-Device-Connection-Type", "WIFI")
                    .header("X-YouTube-Device-Memory-Mb", "6144")
                    .header("X-YouTube-Device-Display-Density", "460")
                    .header("X-YouTube-Device-Display-Height", "2532")
                    .header("X-YouTube-Device-Display-Width", "1170")
                    .header("X-YouTube-Device-Cpu-Cores", "6")
                    .header("X-YouTube-Device-Cpu-Model", "A15 Bionic")
                    .header("X-YouTube-Device-Gpu-Model", "Apple GPU")
                    .header("X-YouTube-Device-Gpu-Version", "Metal")
                    .header("X-YouTube-Device-Screen-Density", "460")
                    .header("X-YouTube-Device-Screen-Height", "2532")
                    .header("X-YouTube-Device-Screen-Width", "1170")
                    .header("X-YouTube-Device-Timezone", "UTC")
                    .header("X-YouTube-Device-Language", "en")
                    .header("X-YouTube-Device-Country", "US")
                    .header("X-YouTube-Device-Region", "US")
                    .header("X-YouTube-Device-Carrier", "Unknown")
                    .header("X-YouTube-Device-Network-Type", "WIFI")
                    .header("X-YouTube-Device-Network-Speed", "1000000")
                    .header("X-YouTube-Device-Network-Signal", "4")
                    .header("X-YouTube-Device-Battery-Level", "100")
                    .header("X-YouTube-Device-Battery-Charging", "false")
                    .header("X-YouTube-Device-Volume", "100")
                    .header("X-YouTube-Device-Brightness", "100")
                    .header("X-YouTube-Device-Orientation", "portrait")
                    .header("X-YouTube-Device-Accelerometer", "true")
                    .header("X-YouTube-Device-Gyroscope", "true")
                    .header("X-YouTube-Device-Magnetometer", "true")
                    .header("X-YouTube-Device-Proximity", "true")
                    .header("X-YouTube-Device-Light", "true")
                    .header("X-YouTube-Device-Pressure", "false")
                    .header("X-YouTube-Device-Temperature", "false")
                    .header("X-YouTube-Device-Humidity", "false")
                    .header("X-YouTube-Device-Altitude", "0")
                    .header("X-YouTube-Device-Latitude", "0")
                    .header("X-YouTube-Device-Longitude", "0")
                    .header("X-YouTube-Device-Accuracy", "0")
                    .header("X-YouTube-Device-Speed", "0")
                    .header("X-YouTube-Device-Heading", "0")
                    .header("X-YouTube-Device-Timestamp", "0");
            }
            _ => {
                // Web clients don't need device headers
            }
        }

        request
    }

    /// Execute request with retry logic and client switching
    pub async fn execute_with_retry<T>(
        &mut self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, RytError>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            debug!(
                "HTTP request attempt {}/{}",
                attempt + 1,
                self.config.max_retries
            );

            match request.try_clone().unwrap().send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!("HTTP request successful");
                        return Ok(response.json().await?);
                    } else if response.status() == 403 {
                        // Check if this is a botguard challenge
                        let response_text = response.text().await.unwrap_or_default();
                        if response_text.contains("botguard") || response_text.contains("challenge")
                        {
                            warn!("Botguard challenge detected");
                            let error =
                                RytError::BotguardError("Botguard challenge detected".to_string());
                            // Try switching client if enabled
                            if self.config.enable_client_switching {
                                self.switch_client_by_strategy(Some(&error));
                            }
                            return Err(error);
                        }
                        warn!("Rate limited (403), switching client");
                        let error = RytError::RateLimited;
                        // Try switching client if enabled
                        if self.config.enable_client_switching {
                            self.switch_client_by_strategy(Some(&error));
                        }
                        return Err(error);
                    } else if response.status() == 404 {
                        warn!("Video unavailable (404), switching client");
                        let error = RytError::VideoUnavailable;
                        // Try switching client if enabled
                        if self.config.enable_client_switching {
                            self.switch_client_by_strategy(Some(&error));
                        }
                        return Err(error);
                    } else {
                        warn!("HTTP request failed with status: {}", response.status());
                        last_error = Some(RytError::DownloadFailed(reqwest::Error::from(
                            response.error_for_status().unwrap_err(),
                        )));
                    }
                }
                Err(e) => {
                    warn!("HTTP request error: {}", e);
                    last_error = Some(RytError::DownloadFailed(e));
                }
            }

            // Exponential backoff
            if attempt < self.config.max_retries - 1 {
                let delay = Duration::from_millis(200 * (1 << attempt));
                debug!("Retrying in {:?}", delay);
                tokio::time::sleep(delay).await;
            }
        }

        error!("All retry attempts failed");
        Err(last_error.unwrap_or(RytError::Generic("Request failed".to_string())))
    }
}

impl Default for VideoClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = VideoClient::new();
        assert_eq!(client.config().timeout, Duration::from_secs(30));
        assert_eq!(client.config().max_retries, 3);
    }

    #[test]
    fn test_client_with_config() {
        let config = HttpClientConfig {
            timeout: Duration::from_secs(60),
            max_retries: 5,
            user_agent: Some("Custom Agent".to_string()),
            proxy_url: None,
            client_type: ClientType::Chrome,
            http1_only: false,
            enable_client_switching: true,
            switching_strategy: ClientSwitchingStrategy::Smart,
        };

        let client = VideoClient::with_config(config);
        assert_eq!(client.config().timeout, Duration::from_secs(60));
        assert_eq!(client.config().max_retries, 5);
        assert_eq!(client.config().user_agent, Some("Custom Agent".to_string()));
    }

    #[test]
    fn test_client_type_all() {
        let all_types = ClientType::all();
        assert_eq!(all_types.len(), 10);
        assert!(all_types.contains(&ClientType::Chrome));
        assert!(all_types.contains(&ClientType::Android));
        assert!(all_types.contains(&ClientType::Ios));
    }

    #[test]
    fn test_client_type_from_str() {
        assert_eq!(ClientType::from_str("chrome"), Some(ClientType::Chrome));
        assert_eq!(ClientType::from_str("Chrome"), Some(ClientType::Chrome));
        assert_eq!(ClientType::from_str("CHROME"), Some(ClientType::Chrome));
        assert_eq!(ClientType::from_str("android"), Some(ClientType::Android));
        assert_eq!(ClientType::from_str("ios"), Some(ClientType::Ios));
        assert_eq!(
            ClientType::from_str("samsung"),
            Some(ClientType::SamsungBrowser)
        );
        assert_eq!(ClientType::from_str("invalid"), None);
    }

    #[test]
    fn test_client_type_to_string() {
        assert_eq!(ClientType::Chrome.to_string(), "chrome");
        assert_eq!(ClientType::Android.to_string(), "android");
        assert_eq!(ClientType::Ios.to_string(), "ios");
        assert_eq!(ClientType::SamsungBrowser.to_string(), "samsung");
    }

    #[test]
    fn test_client_type_is_mobile() {
        assert!(ClientType::Android.is_mobile());
        assert!(ClientType::Ios.is_mobile());
        assert!(ClientType::SamsungBrowser.is_mobile());
        assert!(!ClientType::Chrome.is_mobile());
        assert!(!ClientType::Firefox.is_mobile());
        assert!(!ClientType::AndroidTV.is_mobile());
    }

    #[test]
    fn test_client_type_is_web() {
        assert!(ClientType::Chrome.is_web());
        assert!(ClientType::Firefox.is_web());
        assert!(ClientType::Safari.is_web());
        assert!(ClientType::Edge.is_web());
        assert!(ClientType::Opera.is_web());
        assert!(!ClientType::Android.is_web());
        assert!(!ClientType::Ios.is_web());
    }

    #[test]
    fn test_client_type_is_tv() {
        assert!(ClientType::AndroidTV.is_tv());
        assert!(ClientType::SmartTV.is_tv());
        assert!(!ClientType::Chrome.is_tv());
        assert!(!ClientType::Android.is_tv());
    }

    #[test]
    fn test_client_switching_strategy_default() {
        assert_eq!(
            ClientSwitchingStrategy::default(),
            ClientSwitchingStrategy::Smart
        );
    }

    #[test]
    fn test_http_client_config_default() {
        let config = HttpClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.user_agent, None);
        assert_eq!(config.proxy_url, None);
        assert_eq!(config.client_type, ClientType::Chrome);
        assert!(config.enable_client_switching);
        assert_eq!(config.switching_strategy, ClientSwitchingStrategy::Smart);
        assert!(!config.http1_only);
    }

    #[test]
    fn test_video_client_default() {
        let client = VideoClient::default();
        assert_eq!(client.config().timeout, Duration::from_secs(30));
        assert_eq!(client.config().max_retries, 3);
    }

    #[test]
    fn test_video_client_current_client_type() {
        let client = VideoClient::new();
        assert_eq!(client.current_client_type(), ClientType::Chrome);
    }

    #[test]
    fn test_video_client_client_switch_count() {
        let client = VideoClient::new();
        assert_eq!(client.client_switch_count(), 0);
    }

    #[test]
    fn test_video_client_switch_client() {
        let mut client = VideoClient::new();
        let initial_type = client.current_client_type();
        let new_type = client.switch_client();

        // Should switch to next client in round-robin
        assert_ne!(initial_type, new_type);
        assert_eq!(client.client_switch_count(), 1);
    }

    #[test]
    fn test_video_client_switch_to_client() {
        let mut client = VideoClient::new();
        client.switch_to_client(ClientType::Android);

        assert_eq!(client.current_client_type(), ClientType::Android);
        assert_eq!(client.client_switch_count(), 1);
    }

    #[test]
    fn test_video_client_reset_client_switching() {
        let mut client = VideoClient::new();
        client.switch_client();
        client.switch_client();

        assert!(client.client_switch_count() > 0);

        client.reset_client_switching();

        assert_eq!(client.client_switch_count(), 0);
        assert_eq!(client.current_client_type(), ClientType::Chrome);
    }

    #[test]
    fn test_video_client_switch_client_disabled() {
        let mut config = HttpClientConfig::default();
        config.enable_client_switching = false;
        let mut client = VideoClient::with_config(config);

        let initial_type = client.current_client_type();
        let new_type = client.switch_client();

        // Should not switch when disabled
        assert_eq!(initial_type, new_type);
        assert_eq!(client.client_switch_count(), 0);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_round_robin() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::RoundRobin;
        let mut client = VideoClient::with_config(config);

        let initial_type = client.current_client_type();
        let new_type = client.switch_client_by_strategy(None);

        // Should switch to next client
        assert_ne!(initial_type, new_type);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_on_error() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::OnError;
        let mut client = VideoClient::with_config(config);

        let error = RytError::RateLimited;
        let new_type = client.switch_client_by_strategy(Some(&error));

        // Should switch to Android for rate limiting
        assert_eq!(new_type, ClientType::Android);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_on_geo_block() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::OnGeoBlock;
        let mut client = VideoClient::with_config(config);

        let error = RytError::VideoUnavailable;
        let new_type = client.switch_client_by_strategy(Some(&error));

        // Should switch to Android for geo-blocking
        assert_eq!(new_type, ClientType::Android);
    }

    #[test]
    fn test_video_client_create_request() {
        let client = VideoClient::new();
        let request = client.create_request(reqwest::Method::GET, "https://example.com");

        // Request should be created successfully
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request() {
        let client = VideoClient::new();
        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");

        // Request should be created successfully
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_simple_media_request() {
        let client = VideoClient::new();
        let request =
            client.create_simple_media_request(reqwest::Method::GET, "https://example.com");

        // Request should be created successfully
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request() {
        let client = VideoClient::new();
        let request = client.create_innertube_request("https://example.com");

        // Request should be created successfully
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_android() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Android;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_ios() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Ios;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_chrome() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Chrome;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_firefox() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Firefox;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_safari() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Safari;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_edge() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Edge;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_tv() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::AndroidTV;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_web() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Opera;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_realistic_request_with_embedded() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::SamsungBrowser;
        let client = VideoClient::with_config(config);

        let request = client.create_realistic_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_simple_media_request_with_android() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Android;
        let client = VideoClient::with_config(config);

        let request =
            client.create_simple_media_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_simple_media_request_with_ios() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Ios;
        let client = VideoClient::with_config(config);

        let request =
            client.create_simple_media_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_simple_media_request_with_chrome() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Chrome;
        let client = VideoClient::with_config(config);

        let request =
            client.create_simple_media_request(reqwest::Method::GET, "https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_android() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Android;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_ios() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Ios;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_chrome() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Chrome;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_firefox() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Firefox;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_safari() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Safari;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_edge() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Edge;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_tv() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::AndroidTV;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_web() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::Opera;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_create_innertube_request_with_embedded() {
        let mut config = HttpClientConfig::default();
        config.client_type = ClientType::SamsungBrowser;
        let client = VideoClient::with_config(config);

        let request = client.create_innertube_request("https://example.com");
        assert!(request.try_clone().is_some());
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_round_robin_multiple_calls() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::RoundRobin;
        let mut client = VideoClient::with_config(config);

        let initial_type = client.current_client_type();
        let new_type = client.switch_client_by_strategy(None);

        // Should switch to next client in round robin
        assert_ne!(new_type, initial_type);

        let next_type = client.switch_client_by_strategy(None);
        assert_ne!(next_type, new_type);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_on_error_with_different_errors() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::OnError;
        let mut client = VideoClient::with_config(config);

        // Test with different error types
        let rate_limit_error = RytError::RateLimited;
        let new_type1 = client.switch_client_by_strategy(Some(&rate_limit_error));
        assert_eq!(new_type1, ClientType::Android);

        let geo_block_error = RytError::GeoBlocked;
        let new_type2 = client.switch_client_by_strategy(Some(&geo_block_error));
        assert_eq!(new_type2, ClientType::Android);

        let timeout_error = RytError::TimeoutError("test".to_string());
        let new_type3 = client.switch_client_by_strategy(Some(&timeout_error));
        assert_eq!(new_type3, ClientType::Android);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_no_error() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::OnError;
        let mut client = VideoClient::with_config(config);

        let initial_type = client.current_client_type();
        let new_type = client.switch_client_by_strategy(None);

        // Should not switch when no error
        assert_eq!(new_type, initial_type);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_round_robin_no_error() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::RoundRobin;
        let mut client = VideoClient::with_config(config);

        let initial_type = client.current_client_type();
        let new_type = client.switch_client_by_strategy(None);

        // Should switch in round robin even without error
        assert_ne!(new_type, initial_type);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_on_geo_block_no_error() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::OnGeoBlock;
        let mut client = VideoClient::with_config(config);

        let initial_type = client.current_client_type();
        let new_type = client.switch_client_by_strategy(None);

        // Should not switch when no geo block error
        assert_eq!(new_type, initial_type);
    }

    #[test]
    fn test_video_client_switch_client_by_strategy_on_geo_block_wrong_error() {
        let mut config = HttpClientConfig::default();
        config.switching_strategy = ClientSwitchingStrategy::OnGeoBlock;
        let mut client = VideoClient::with_config(config);

        let rate_limit_error = RytError::RateLimited;
        let initial_type = client.current_client_type();
        let new_type = client.switch_client_by_strategy(Some(&rate_limit_error));

        // Should not switch for non-geo block errors
        assert_eq!(new_type, initial_type);
    }
}
