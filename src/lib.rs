//! # ryt - Rust YouTube Downloader
//!
//! Fast and reliable YouTube video downloader written in Rust.
//! 
//! ## Features
//!
//! - High-performance chunked downloading
//! - YouTube signature deciphering
//! - Botguard protection bypass
//! - Multiple format selection
//! - Playlist support
//! - Rate limiting and retry logic
//!
//! ## Example
//!
//! ```rust,no_run
//! use ryt::Downloader;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut downloader = Downloader::new()
//!         .with_format("best", "mp4")
//!         .with_output_path("./downloads");
//!     
//!     let info = downloader.download("VIDEO_URL").await?;
//!     println!("Downloaded: {}", info.title);
//!     
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod core;
pub mod download;
pub mod error;
pub mod utils;
pub mod platform;

// Re-export main types
pub use core::{Downloader, DownloadOptions, VideoInfo, Progress, Format, FormatSelector, QualitySelector};
pub use error::RytError;

/// Result type alias for ryt operations
pub type Result<T> = std::result::Result<T, RytError>;
