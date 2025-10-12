//! Progress tracking for downloads

use std::time::{Duration, Instant};

/// Progress information for a download
#[derive(Debug, Clone)]
pub struct Progress {
    /// Total size of the file in bytes
    pub total_size: u64,
    /// Number of bytes downloaded
    pub downloaded_size: u64,
    /// Download progress as a percentage (0.0 to 100.0)
    pub percent: f64,
    /// Current download speed in bytes per second
    pub speed: Option<f64>,
    /// Estimated time remaining
    pub eta: Option<Duration>,
    /// Time when download started
    pub start_time: Instant,
}

impl Progress {
    /// Create a new progress tracker
    pub fn new(total_size: u64) -> Self {
        Self {
            total_size,
            downloaded_size: 0,
            percent: 0.0,
            speed: None,
            eta: None,
            start_time: Instant::now(),
        }
    }

    /// Update progress with new downloaded size
    pub fn update(&mut self, downloaded_size: u64) {
        self.downloaded_size = downloaded_size;
        self.percent = if self.total_size > 0 {
            (downloaded_size as f64 / self.total_size as f64) * 100.0
        } else {
            0.0
        };

        // Calculate speed and ETA
        let elapsed = self.start_time.elapsed();
        if elapsed.as_millis() > 0 {
            self.speed = Some(downloaded_size as f64 / elapsed.as_secs_f64());

            if let Some(speed) = self.speed {
                if speed > 0.0 && self.total_size > downloaded_size {
                    let remaining_bytes = self.total_size - downloaded_size;
                    self.eta = Some(Duration::from_secs((remaining_bytes as f64 / speed) as u64));
                }
            }
        }
    }

    /// Check if download is complete
    pub fn is_complete(&self) -> bool {
        self.total_size > 0 && self.downloaded_size >= self.total_size
    }

    /// Get human-readable speed string
    pub fn speed_string(&self) -> String {
        if let Some(speed) = self.speed {
            format_bytes_per_second(speed)
        } else {
            "Unknown".to_string()
        }
    }

    /// Get human-readable ETA string
    pub fn eta_string(&self) -> String {
        if let Some(eta) = self.eta {
            format_duration(eta)
        } else {
            "Unknown".to_string()
        }
    }

    /// Get human-readable total size string
    pub fn total_size_string(&self) -> String {
        format_bytes(self.total_size)
    }

    /// Get human-readable downloaded size string
    pub fn downloaded_size_string(&self) -> String {
        format_bytes(self.downloaded_size)
    }
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
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

/// Format bytes per second as human-readable string
pub fn format_bytes_per_second(bytes_per_second: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_second as u64))
}

/// Format duration as human-readable string
pub fn format_duration(duration: Duration) -> String {
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
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_progress_creation() {
        let progress = Progress::new(1000);
        assert_eq!(progress.total_size, 1000);
        assert_eq!(progress.downloaded_size, 0);
        assert_eq!(progress.percent, 0.0);
        assert!(!progress.is_complete());
    }

    #[test]
    fn test_progress_update() {
        let mut progress = Progress::new(1000);

        progress.update(500);
        assert_eq!(progress.downloaded_size, 500);
        assert_eq!(progress.percent, 50.0);
        assert!(!progress.is_complete());

        progress.update(1000);
        assert_eq!(progress.downloaded_size, 1000);
        assert_eq!(progress.percent, 100.0);
        assert!(progress.is_complete());
    }

    #[test]
    fn test_progress_speed_calculation() {
        let mut progress = Progress::new(1000);

        // Simulate some time passing
        thread::sleep(Duration::from_millis(100));
        progress.update(100);

        // Speed should be calculated
        assert!(progress.speed.is_some());
        assert!(progress.speed.unwrap() > 0.0);
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
}
