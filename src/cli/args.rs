//! Command line argument parsing

use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::time::Duration;

/// Rust YouTube Downloader - Fast and reliable YouTube video downloader
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// YouTube video or playlist URL
    pub url: String,

    /// Format selector (e.g., 'itag=22', 'best', 'height<=480')
    #[arg(short, long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// Desired file extension (e.g., 'mp4', 'webm')
    #[arg(short, long, value_name = "EXT")]
    pub ext: Option<String>,

    /// Output path (file or directory)
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Disable progress output
    #[arg(long)]
    pub no_progress: bool,

    /// HTTP timeout (e.g., 30s, 1m)
    #[arg(long, value_name = "DURATION", default_value = "30s")]
    pub timeout: humantime::Duration,

    /// HTTP retries for transient errors
    #[arg(long, default_value = "3")]
    pub retries: u32,

    /// Download rate limit (e.g., 2MiB/s, 500KiB/s)
    #[arg(long, value_name = "RATE")]
    pub rate_limit: Option<String>,

    /// Treat input as playlist URL or ID
    #[arg(long)]
    pub playlist: bool,

    /// Max items to process for playlist (0 means all)
    #[arg(long, default_value = "0")]
    pub limit: usize,

    /// Parallelism for playlist downloads
    #[arg(long, default_value = "1")]
    pub concurrency: usize,

    /// Botguard mode
    #[arg(long, value_enum, default_value = "off")]
    pub botguard: BotguardMode,

    /// Enable Botguard debug logs
    #[arg(long)]
    pub debug_botguard: bool,

    /// Botguard cache mode
    #[arg(long, value_enum, default_value = "mem")]
    pub botguard_cache: BotguardCacheMode,

    /// Botguard cache directory (for file mode)
    #[arg(long, value_name = "DIR")]
    pub botguard_cache_dir: Option<PathBuf>,

    /// Default Botguard token TTL if solver doesn't set
    #[arg(long, value_name = "DURATION", default_value = "30m")]
    pub botguard_ttl: humantime::Duration,

    /// Path to JS script implementing bgAttest(input)
    #[arg(long, value_name = "PATH")]
    pub botguard_script: Option<PathBuf>,

    /// Innertube client name (default ANDROID)
    #[arg(long, value_name = "NAME")]
    pub client_name: Option<String>,

    /// Innertube client version (default 20.10.38)
    #[arg(long, value_name = "VERSION")]
    pub client_version: Option<String>,

    /// Print final media URL and exit (no download)
    #[arg(short = 'g', long)]
    pub print_url: bool,

    /// Override User-Agent header
    #[arg(long, value_name = "USER_AGENT")]
    pub user_agent: Option<String>,

    /// Proxy URL (http/https/socks)
    #[arg(long, value_name = "URL")]
    pub proxy: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Quiet output (only errors)
    #[arg(short, long)]
    pub quiet: bool,
}

/// Botguard mode
#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum BotguardMode {
    /// Disabled
    Off,
    /// Automatic (only when needed)
    Auto,
    /// Force (always use)
    Force,
}

/// Botguard cache mode
#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum BotguardCacheMode {
    /// Memory cache
    Mem,
    /// File cache
    File,
}

impl Args {
    /// Get HTTP timeout as Duration
    pub fn timeout_duration(&self) -> Duration {
        self.timeout.into()
    }

    /// Get Botguard TTL as Duration
    pub fn botguard_ttl_duration(&self) -> Duration {
        self.botguard_ttl.into()
    }

    /// Parse rate limit string to bytes per second
    pub fn parse_rate_limit(&self) -> Option<u64> {
        self.rate_limit
            .as_ref()
            .and_then(|rate| parse_rate_limit(rate))
    }

    /// Check if this is a playlist operation
    pub fn is_playlist(&self) -> bool {
        self.playlist || crate::utils::url::is_playlist_url(&self.url)
    }

    /// Get output verbosity level
    pub fn verbosity_level(&self) -> VerbosityLevel {
        if self.quiet {
            VerbosityLevel::Quiet
        } else if self.verbose {
            VerbosityLevel::Verbose
        } else {
            VerbosityLevel::Normal
        }
    }
}

/// Output verbosity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbosityLevel {
    /// Quiet (only errors)
    Quiet,
    /// Normal
    Normal,
    /// Verbose (debug info)
    Verbose,
}

/// Parse rate limit string to bytes per second
pub fn parse_rate_limit(rate: &str) -> Option<u64> {
    let rate = rate.trim().to_uppercase();
    if rate.is_empty() {
        return None;
    }

    // Remove /s suffix if present
    let rate = rate.trim_end_matches("/S");

    // Find the number and unit
    let mut number_end = 0;
    for (i, c) in rate.char_indices() {
        if c.is_ascii_digit() || c == '.' {
            number_end = i + 1;
        } else {
            break;
        }
    }

    if number_end == 0 {
        return None;
    }

    let number_str = &rate[..number_end];
    let unit = &rate[number_end..].trim();

    let number: f64 = number_str.parse().ok()?;
    if number <= 0.0 {
        return None;
    }

    let multiplier = match *unit {
        "B" | "" => 1,
        "KB" => 1000,
        "KIB" => 1024,
        "MB" => 1000 * 1000,
        "MIB" => 1024 * 1024,
        "GB" => 1000 * 1000 * 1000,
        "GIB" => 1024 * 1024 * 1024,
        "TB" => 1000_u64.pow(4),
        "TIB" => 1024_u64.pow(4),
        _ => return None,
    };

    Some((number * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rate_limit() {
        assert_eq!(parse_rate_limit("1MB/s"), Some(1000 * 1000));
        assert_eq!(parse_rate_limit("1MiB/s"), Some(1024 * 1024));
        assert_eq!(parse_rate_limit("500KB/s"), Some(500 * 1000));
        assert_eq!(parse_rate_limit("2GB/s"), Some(2 * 1000 * 1000 * 1000));
        assert_eq!(parse_rate_limit("1.5MB/s"), Some(1500 * 1000));
        assert_eq!(parse_rate_limit("1024"), Some(1024));
        assert_eq!(parse_rate_limit("0"), None);
        assert_eq!(parse_rate_limit(""), None);
        assert_eq!(parse_rate_limit("invalid"), None);
    }

    #[test]
    fn test_args_verbosity_level() {
        let args = Args {
            url: "https://example.com".to_string(),
            quiet: false,
            verbose: false,
            ..Default::default()
        };
        assert_eq!(args.verbosity_level(), VerbosityLevel::Normal);

        let args = Args {
            url: "https://example.com".to_string(),
            quiet: true,
            verbose: false,
            ..Default::default()
        };
        assert_eq!(args.verbosity_level(), VerbosityLevel::Quiet);

        let args = Args {
            url: "https://example.com".to_string(),
            quiet: false,
            verbose: true,
            ..Default::default()
        };
        assert_eq!(args.verbosity_level(), VerbosityLevel::Verbose);
    }

    #[test]
    fn test_args_is_playlist() {
        let args = Args {
            url: "https://www.youtube.com/playlist?list=PLxxxx".to_string(),
            playlist: false,
            ..Default::default()
        };
        assert!(args.is_playlist());

        let args = Args {
            url: "https://www.youtube.com/watch?v=xxx".to_string(),
            playlist: true,
            ..Default::default()
        };
        assert!(args.is_playlist());
    }

    #[test]
    fn test_args_timeout_duration() {
        let args = Args {
            timeout: humantime::Duration::from(Duration::from_secs(60)),
            ..Default::default()
        };
        assert_eq!(args.timeout_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_args_botguard_ttl_duration() {
        let args = Args {
            botguard_ttl: humantime::Duration::from(Duration::from_secs(3600)),
            ..Default::default()
        };
        assert_eq!(args.botguard_ttl_duration(), Duration::from_secs(3600));
    }

    #[test]
    fn test_args_parse_rate_limit() {
        let args = Args {
            rate_limit: Some("1MB/s".to_string()),
            ..Default::default()
        };
        assert_eq!(args.parse_rate_limit(), Some(1000 * 1000));

        let args = Args {
            rate_limit: None,
            ..Default::default()
        };
        assert_eq!(args.parse_rate_limit(), None);
    }

    #[test]
    fn test_parse_rate_limit_edge_cases() {
        // Test various units
        assert_eq!(parse_rate_limit("1B"), Some(1));
        assert_eq!(parse_rate_limit("1KB"), Some(1000));
        assert_eq!(parse_rate_limit("1KiB"), Some(1024));
        assert_eq!(parse_rate_limit("1MB"), Some(1000 * 1000));
        assert_eq!(parse_rate_limit("1MiB"), Some(1024 * 1024));
        assert_eq!(parse_rate_limit("1GB"), Some(1000 * 1000 * 1000));
        assert_eq!(parse_rate_limit("1GiB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_rate_limit("1TB"), Some(1000_u64.pow(4)));
        assert_eq!(parse_rate_limit("1TiB"), Some(1024_u64.pow(4)));

        // Test decimal numbers
        assert_eq!(parse_rate_limit("1.5MB"), Some(1500 * 1000));
        assert_eq!(parse_rate_limit("0.5MB"), Some(500 * 1000));

        // Test case insensitive
        assert_eq!(parse_rate_limit("1mb/s"), Some(1000 * 1000));
        assert_eq!(parse_rate_limit("1MB/S"), Some(1000 * 1000));

        // Test whitespace
        assert_eq!(parse_rate_limit(" 1MB/s "), Some(1000 * 1000));

        // Test invalid cases
        assert_eq!(parse_rate_limit("0MB"), None);
        assert_eq!(parse_rate_limit("-1MB"), None);
        assert_eq!(parse_rate_limit("invalid"), None);
        assert_eq!(parse_rate_limit("1XB"), None);
    }

    #[test]
    fn test_botguard_mode_variants() {
        // Test that variants can be created and compared
        assert_eq!(BotguardMode::Off, BotguardMode::Off);
        assert_eq!(BotguardMode::Auto, BotguardMode::Auto);
        assert_eq!(BotguardMode::Force, BotguardMode::Force);
    }

    #[test]
    fn test_botguard_cache_mode_variants() {
        // Test that variants can be created and compared
        assert_eq!(BotguardCacheMode::Mem, BotguardCacheMode::Mem);
        assert_eq!(BotguardCacheMode::File, BotguardCacheMode::File);
    }

    #[test]
    fn test_verbosity_level_variants() {
        assert_eq!(VerbosityLevel::Quiet, VerbosityLevel::Quiet);
        assert_eq!(VerbosityLevel::Normal, VerbosityLevel::Normal);
        assert_eq!(VerbosityLevel::Verbose, VerbosityLevel::Verbose);
    }

    #[test]
    fn test_args_default_values() {
        let args = Args::default();
        assert_eq!(args.url, "");
        assert_eq!(args.format, None);
        assert_eq!(args.ext, None);
        assert_eq!(args.output, None);
        assert_eq!(args.no_progress, false);
        assert_eq!(args.retries, 3);
        assert_eq!(args.rate_limit, None);
        assert_eq!(args.playlist, false);
        assert_eq!(args.limit, 0);
        assert_eq!(args.concurrency, 1);
        assert_eq!(args.botguard, BotguardMode::Off);
        assert_eq!(args.debug_botguard, false);
        assert_eq!(args.botguard_cache, BotguardCacheMode::Mem);
        assert_eq!(args.botguard_cache_dir, None);
        assert_eq!(args.botguard_script, None);
        assert_eq!(args.client_name, None);
        assert_eq!(args.client_version, None);
        assert_eq!(args.print_url, false);
        assert_eq!(args.user_agent, None);
        assert_eq!(args.proxy, None);
        assert_eq!(args.verbose, false);
        assert_eq!(args.quiet, false);
    }

    #[test]
    fn test_args_custom_values() {
        let args = Args {
            url: "https://example.com".to_string(),
            format: Some("best".to_string()),
            ext: Some("mp4".to_string()),
            output: Some(PathBuf::from("/tmp")),
            no_progress: true,
            retries: 5,
            rate_limit: Some("2MB/s".to_string()),
            playlist: true,
            limit: 10,
            concurrency: 3,
            botguard: BotguardMode::Auto,
            debug_botguard: true,
            botguard_cache: BotguardCacheMode::File,
            botguard_cache_dir: Some(PathBuf::from("/cache")),
            botguard_script: Some(PathBuf::from("/script.js")),
            client_name: Some("CHROME".to_string()),
            client_version: Some("1.0.0".to_string()),
            print_url: true,
            user_agent: Some("Custom Agent".to_string()),
            proxy: Some("http://proxy:8080".to_string()),
            verbose: true,
            quiet: false,
            ..Default::default()
        };

        assert_eq!(args.url, "https://example.com");
        assert_eq!(args.format, Some("best".to_string()));
        assert_eq!(args.ext, Some("mp4".to_string()));
        assert_eq!(args.output, Some(PathBuf::from("/tmp")));
        assert_eq!(args.no_progress, true);
        assert_eq!(args.retries, 5);
        assert_eq!(args.rate_limit, Some("2MB/s".to_string()));
        assert_eq!(args.playlist, true);
        assert_eq!(args.limit, 10);
        assert_eq!(args.concurrency, 3);
        assert_eq!(args.botguard, BotguardMode::Auto);
        assert_eq!(args.debug_botguard, true);
        assert_eq!(args.botguard_cache, BotguardCacheMode::File);
        assert_eq!(args.botguard_cache_dir, Some(PathBuf::from("/cache")));
        assert_eq!(args.botguard_script, Some(PathBuf::from("/script.js")));
        assert_eq!(args.client_name, Some("CHROME".to_string()));
        assert_eq!(args.client_version, Some("1.0.0".to_string()));
        assert_eq!(args.print_url, true);
        assert_eq!(args.user_agent, Some("Custom Agent".to_string()));
        assert_eq!(args.proxy, Some("http://proxy:8080".to_string()));
        assert_eq!(args.verbose, true);
        assert_eq!(args.quiet, false);
    }
}

// Implement Default for Args to make tests work
impl Default for Args {
    fn default() -> Self {
        Self {
            url: String::new(),
            format: None,
            ext: None,
            output: None,
            no_progress: false,
            timeout: humantime::Duration::from(Duration::from_secs(30)),
            retries: 3,
            rate_limit: None,
            playlist: false,
            limit: 0,
            concurrency: 1,
            botguard: BotguardMode::Off,
            debug_botguard: false,
            botguard_cache: BotguardCacheMode::Mem,
            botguard_cache_dir: None,
            botguard_ttl: humantime::Duration::from(Duration::from_secs(1800)),
            botguard_script: None,
            client_name: None,
            client_version: None,
            print_url: false,
            user_agent: None,
            proxy: None,
            verbose: false,
            quiet: false,
        }
    }
}
