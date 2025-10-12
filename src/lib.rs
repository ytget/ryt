//! # ryt - Rust Video Downloader
//!
//! Fast and reliable video downloader written in Rust.
//!
//! NOTE: Temporarily allowing some clippy lints for existing code issues

#![allow(clippy::should_implement_trait)]
#![allow(clippy::manual_strip)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::new_without_default)]
#![allow(clippy::len_zero)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::inherent_to_string)]
#![allow(clippy::useless_vec)]
#![allow(clippy::unnecessary_map_or)]
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
pub mod platform;
pub mod utils;

// Re-export main types
pub use core::{
    DownloadOptions, Downloader, Format, FormatSelector, Progress, QualitySelector, VideoInfo,
};
pub use error::RytError;

/// Result type alias for ryt operations
pub type Result<T> = std::result::Result<T, RytError>;
