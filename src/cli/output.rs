//! Output formatting and progress display

use crate::cli::args::VerbosityLevel;
use crate::core::progress::Progress;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;

/// Output formatter for ryt
pub struct OutputFormatter {
    verbosity: VerbosityLevel,
    progress_bar: Option<ProgressBar>,
}

impl OutputFormatter {
    /// Create a new output formatter
    pub fn new(verbosity: VerbosityLevel) -> Self {
        Self {
            verbosity,
            progress_bar: None,
        }
    }

    /// Create a progress bar for downloads
    pub fn create_progress_bar(&mut self, total_size: u64) -> Option<ProgressBar> {
        if self.verbosity == VerbosityLevel::Quiet {
            return None;
        }

        let style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-");

        let progress_bar = ProgressBar::new(total_size);
        progress_bar.set_style(style);
        progress_bar.set_message("Downloading...");

        self.progress_bar = Some(progress_bar.clone());
        Some(progress_bar)
    }

    /// Update progress bar
    pub fn update_progress(&self, progress: &Progress) {
        if let Some(progress_bar) = &self.progress_bar {
            progress_bar.set_position(progress.downloaded_size);
            progress_bar.set_length(progress.total_size);

            if let Some(speed) = progress.speed {
                progress_bar.set_message(format!("{}/s", format_bytes(speed as u64)));
            }
        }
    }

    /// Finish progress bar
    pub fn finish_progress(&self, message: &str) {
        if let Some(progress_bar) = &self.progress_bar {
            progress_bar.finish_with_message(message.to_string());
        }
    }

    /// Print info message
    pub fn info(&self, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet {
            println!("ℹ️  {}", message);
        }
    }

    /// Print success message
    pub fn success(&self, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet {
            println!("✅ {}", message);
        }
    }

    /// Print warning message
    pub fn warning(&self, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet {
            eprintln!("⚠️  {}", message);
        }
    }

    /// Print error message
    pub fn error(&self, message: &str) {
        eprintln!("❌ {}", message);
    }

    /// Print debug message
    pub fn debug(&self, message: &str) {
        if self.verbosity == VerbosityLevel::Verbose {
            println!("🐛 {}", message);
        }
    }

    /// Print video information
    pub fn print_video_info(&self, title: &str, author: &str, duration: u32, formats: usize) {
        if self.verbosity == VerbosityLevel::Quiet {
            return;
        }

        println!("📹 {}", title);
        println!("👤 {}", author);
        println!(
            "⏱️  {}",
            format_duration(Duration::from_secs(duration as u64))
        );
        println!("📊 {} formats available", formats);
        println!();
    }

    /// Print format information
    pub fn print_format_info(
        &self,
        itag: u32,
        quality: &str,
        mime_type: &str,
        bitrate: u32,
        size: Option<u64>,
    ) {
        if self.verbosity == VerbosityLevel::Quiet {
            return;
        }

        let size_str = size
            .map(|s| format!(" ({})", format_bytes(s)))
            .unwrap_or_default();
        println!(
            "  📋 itag={} | {} | {} | {} kbps{}",
            itag,
            quality,
            mime_type,
            bitrate / 1000,
            size_str
        );
    }

    /// Print download start message
    pub fn print_download_start(&self, url: &str, output_path: &str) {
        if self.verbosity == VerbosityLevel::Quiet {
            return;
        }

        println!("🚀 Starting download...");
        println!("🔗 URL: {}", url);
        println!("💾 Output: {}", output_path);
        println!();
    }

    /// Print download complete message
    pub fn print_download_complete(&self, output_path: &str, duration: Duration) {
        if self.verbosity == VerbosityLevel::Quiet {
            return;
        }

        println!();
        println!("✅ Download completed!");
        println!("💾 Saved to: {}", output_path);
        println!("⏱️  Time: {}", format_duration(duration));
    }

    /// Print playlist information
    pub fn print_playlist_info(&self, playlist_id: &str, item_count: usize, limit: Option<usize>) {
        if self.verbosity == VerbosityLevel::Quiet {
            return;
        }

        println!("📋 Playlist: {}", playlist_id);
        if let Some(limit) = limit {
            println!("📊 Items: {} (limited to {})", item_count, limit);
        } else {
            println!("📊 Items: {}", item_count);
        }
        println!();
    }

    /// Print playlist item progress
    pub fn print_playlist_item(&self, index: usize, total: usize, title: &str) {
        if self.verbosity == VerbosityLevel::Quiet {
            return;
        }

        println!("📥 [{}/{}] {}", index + 1, total, title);
    }

    /// Print help text
    pub fn print_help(&self) {
        println!("RYT - Rust Video Downloader");
        println!();
        println!("Usage: ryt [OPTIONS] <URL>");
        println!();
        println!("Examples:");
        println!("  ryt VIDEO_URL");
        println!("  ryt --format best --ext mp4 VIDEO_URL");
        println!("  ryt --playlist --limit 10 PLAYLIST_URL");
        println!("  ryt --rate-limit 2MiB/s --output ./downloads VIDEO_URL");
        println!();
        println!("For more information, run: ryt --help");
    }

    /// Print version information
    pub fn print_version(&self) {
        println!("ryt version {}", env!("CARGO_PKG_VERSION"));
        println!("Rust YouTube Downloader - Fast and reliable");
    }
}

/// Create a progress callback for the downloader
pub fn create_progress_callback(
    formatter: Arc<OutputFormatter>,
) -> impl Fn(Progress) + Send + Sync + 'static {
    move |progress: Progress| {
        formatter.update_progress(&progress);
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f64 = bytes as f64;
    let exp = (bytes_f64.ln() / THRESHOLD.ln()).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);

    let value = bytes_f64 / THRESHOLD.powi(exp as i32);

    if exp == 0 {
        format!("{} {}", bytes, UNITS[exp])
    } else {
        format!("{:.1} {}", value, UNITS[exp])
    }
}

/// Format duration as human-readable string
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();

    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        if seconds == 0 {
            format!("{}m", minutes)
        } else {
            format!("{}m {}s", minutes, seconds)
        }
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        if minutes == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h {}m", hours, minutes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_formatter_creation() {
        let formatter = OutputFormatter::new(VerbosityLevel::Normal);
        assert_eq!(formatter.verbosity, VerbosityLevel::Normal);
        assert!(formatter.progress_bar.is_none());
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(3660)), "1h 1m");
    }

    #[test]
    fn test_verbosity_levels() {
        let formatter = OutputFormatter::new(VerbosityLevel::Quiet);
        // These should not print anything in quiet mode
        formatter.info("test");
        formatter.success("test");
        formatter.warning("test");
        formatter.debug("test");

        // Error should always print
        formatter.error("test");
    }
}
