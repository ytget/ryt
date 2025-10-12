//! Main entry point for ryt CLI

use ryt::cli::Args;
use ryt::core::{Downloader, Progress};
use ryt::platform::botguard::BotguardMode;
use ryt::cli::output::OutputFormatter;
use clap::Parser;
use std::sync::Arc;
use std::time::Instant;
use tokio;
use tracing::{info, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging()?;
    
    // Parse command line arguments
    let args = Args::parse();
    
    info!("Starting ryt with args: {:?}", args);

    // Initialize output formatter
    let formatter = Arc::new(OutputFormatter::new(args.verbosity_level()));

    // Handle special commands
    if args.url.is_empty() {
        formatter.print_help();
        return Ok(());
    }

    // Create downloader
    let mut downloader = Downloader::new();

    // Configure format
    if let (Some(format), Some(ext)) = (&args.format, &args.ext) {
        downloader = downloader.with_format(format, ext);
    } else if let Some(format) = &args.format {
        downloader = downloader.with_format(format, "mp4");
    } else if let Some(ext) = &args.ext {
        downloader = downloader.with_format("best", ext);
    }

    // Configure output path
    if let Some(output) = &args.output {
        downloader = downloader.with_output_path(output);
    }

    // Configure rate limit
    if let Some(rate_limit) = args.parse_rate_limit() {
        downloader = downloader.with_rate_limit(rate_limit);
    }

    // Configure InnerTube client
    if let (Some(name), Some(version)) = (&args.client_name, &args.client_version) {
        downloader = downloader.with_innertube_client(name, version);
    }

    // Configure Botguard
    let botguard_mode = match args.botguard {
        ryt::cli::args::BotguardMode::Off => BotguardMode::Off,
        ryt::cli::args::BotguardMode::Auto => BotguardMode::Auto,
        ryt::cli::args::BotguardMode::Force => BotguardMode::Force,
    };

    downloader = downloader
        .with_botguard(botguard_mode)
        .with_botguard_debug(args.debug_botguard)
        .with_botguard_ttl(args.botguard_ttl_duration());

    // Configure timeout and retries
    downloader = downloader
        .with_timeout(args.timeout_duration())
        .with_max_retries(args.retries);

    // Configure progress callback
    if !args.no_progress {
        let formatter_clone = formatter.clone();
        downloader = downloader.with_progress(move |progress: Progress| {
            formatter_clone.update_progress(&progress);
        });
    }

    // Handle playlist downloads
    if args.is_playlist() {
        return handle_playlist_download(downloader, &args, formatter).await;
    }

    // Handle single video download
    handle_single_download(downloader, &args, formatter).await
}

/// Handle single video download
async fn handle_single_download(
    mut downloader: Downloader,
    args: &Args,
    formatter: Arc<OutputFormatter>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    // Print URL only mode
    if args.print_url {
        debug!("Print URL mode enabled");
        let (final_url, _video_info) = downloader.resolve_url(&args.url).await?;
        println!("{}", final_url);
        return Ok(());
    }

    // Print download start
    formatter.print_download_start(&args.url, "auto-generated filename");
    info!("Starting download for URL: {}", args.url);

    // Download video
    let video_info = downloader.download(&args.url).await?;
    info!("Download completed successfully");

    // Print completion
    let duration = start_time.elapsed();
    formatter.print_download_complete("downloaded file", duration);

    // Print video info
    formatter.print_video_info(
        &video_info.title,
        &video_info.author,
        video_info.duration,
        video_info.formats.len(),
    );

    Ok(())
}

/// Handle playlist download
async fn handle_playlist_download(
    mut downloader: Downloader,
    args: &Args,
    formatter: Arc<OutputFormatter>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    // Extract playlist ID
    let playlist_id = ryt::utils::url::extract_playlist_id(&args.url)?;
    info!("Processing playlist: {}", playlist_id);

    // Print playlist info
    formatter.print_playlist_info(&playlist_id, 0, Some(args.limit));

    // Download playlist
    let limit = if args.limit > 0 { Some(args.limit) } else { None };
    let video_infos = downloader.download_playlist(&args.url, limit).await?;
    info!("Playlist download completed: {} videos", video_infos.len());

    // Print completion
    let duration = start_time.elapsed();
    formatter.success(&format!("Downloaded {} videos in {}", video_infos.len(), format_duration(duration)));

    // Print summary
    for (index, video_info) in video_infos.iter().enumerate() {
        formatter.print_playlist_item(index, video_infos.len(), &video_info.title);
    }

    Ok(())
}

/// Initialize logging system
fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Get log level from environment or default to info
    let log_level = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "info".to_string());
    
    // Parse log level
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level));
    
    // Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .compact())
        .init();
    
    Ok(())
}

/// Format duration as human-readable string
fn format_duration(duration: std::time::Duration) -> String {
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