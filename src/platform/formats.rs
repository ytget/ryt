//! Format parsing and selection utilities

use crate::core::video_info::{Format, FormatSelector, QualitySelector};
use crate::error::RytError;

/// Select the best format based on selector criteria
pub fn select_format<'a>(
    formats: &'a [Format],
    selector: &FormatSelector,
) -> Result<&'a Format, RytError> {
    let mut candidates: Vec<&Format> = formats.iter().collect();

    // Filter by extension
    if let Some(ext) = &selector.extension {
        candidates.retain(|f| f.mime_type.contains(ext));
    }

    // Filter by height constraints
    if let Some(height_limit) = selector.height_limit {
        candidates.retain(|f| {
            if let Some(height) = f.height {
                height <= height_limit
            } else {
                false
            }
        });
    }

    if let Some(height_min) = selector.height_min {
        candidates.retain(|f| {
            if let Some(height) = f.height {
                height >= height_min
            } else {
                false
            }
        });
    }

    // Filter by preferred itag
    if let Some(preferred_itag) = selector.preferred_itag {
        candidates.retain(|f| f.itag == preferred_itag);
    }

    if candidates.is_empty() {
        return Err(RytError::NoFormatFound);
    }

    // Select by quality criteria
    match &selector.quality {
        QualitySelector::Best => {
            // Prioritize progressive formats (video+audio combined)
            if let Some(progressive) = candidates.iter().find(|f| f.is_progressive()) {
                return Ok(progressive);
            }
            // Then sort by bitrate
            candidates.sort_by(|a, b| b.bitrate.cmp(&a.bitrate));
            Ok(candidates.first().unwrap())
        }
        QualitySelector::Worst => {
            candidates.sort_by(|a, b| a.bitrate.cmp(&b.bitrate));
            Ok(candidates.first().unwrap())
        }
        QualitySelector::Itag(target_itag) => candidates
            .iter()
            .find(|f| f.itag == *target_itag)
            .copied()
            .ok_or(RytError::NoFormatFound),
        QualitySelector::Height(target_height) => candidates
            .iter()
            .filter(|f| f.height.unwrap_or(0) == *target_height)
            .max_by_key(|f| f.bitrate)
            .copied()
            .ok_or(RytError::NoFormatFound),
        QualitySelector::HeightLessOrEqual(target_height) => candidates
            .iter()
            .filter(|f| f.height.unwrap_or(0) <= *target_height)
            .max_by_key(|f| f.bitrate)
            .copied()
            .ok_or(RytError::NoFormatFound),
        QualitySelector::HeightGreaterOrEqual(target_height) => candidates
            .iter()
            .filter(|f| f.height.unwrap_or(0) >= *target_height)
            .max_by_key(|f| f.bitrate)
            .copied()
            .ok_or(RytError::NoFormatFound),
    }
}

/// Get the best progressive format (video+audio combined)
pub fn get_best_progressive_format(formats: &[Format]) -> Option<&Format> {
    formats
        .iter()
        .filter(|f| f.is_progressive())
        .max_by_key(|f| f.bitrate)
}

/// Get the best video-only format
pub fn get_best_video_format(formats: &[Format]) -> Option<&Format> {
    formats
        .iter()
        .filter(|f| f.is_video_only())
        .max_by_key(|f| f.bitrate)
}

/// Get the best audio-only format
pub fn get_best_audio_format(formats: &[Format]) -> Option<&Format> {
    formats
        .iter()
        .filter(|f| f.is_audio_only())
        .max_by_key(|f| f.bitrate)
}

/// Get formats by container type
pub fn get_formats_by_container<'a>(formats: &'a [Format], container: &str) -> Vec<&'a Format> {
    formats
        .iter()
        .filter(|f| f.container() == container)
        .collect()
}

/// Get formats by quality
pub fn get_formats_by_quality<'a>(formats: &'a [Format], quality: &str) -> Vec<&'a Format> {
    formats.iter().filter(|f| f.quality == quality).collect()
}

/// Get formats by height range
pub fn get_formats_by_height_range(
    formats: &[Format],
    min_height: u32,
    max_height: u32,
) -> Vec<&Format> {
    formats
        .iter()
        .filter(|f| {
            if let Some(height) = f.height {
                height >= min_height && height <= max_height
            } else {
                false
            }
        })
        .collect()
}

/// Get formats by bitrate range
pub fn get_formats_by_bitrate_range(
    formats: &[Format],
    min_bitrate: u32,
    max_bitrate: u32,
) -> Vec<&Format> {
    formats
        .iter()
        .filter(|f| f.bitrate >= min_bitrate && f.bitrate <= max_bitrate)
        .collect()
}

/// Sort formats by quality (best first)
pub fn sort_formats_by_quality(formats: &mut [Format]) {
    formats.sort_by(|a, b| {
        // First by height (if available)
        match (a.height, b.height) {
            (Some(a_h), Some(b_h)) => b_h.cmp(&a_h),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.bitrate.cmp(&a.bitrate),
        }
    });
}

/// Sort formats by bitrate (highest first)
pub fn sort_formats_by_bitrate(formats: &mut [Format]) {
    formats.sort_by(|a, b| b.bitrate.cmp(&a.bitrate));
}

/// Sort formats by size (largest first)
pub fn sort_formats_by_size(formats: &mut [Format]) {
    formats.sort_by(|a, b| match (a.size, b.size) {
        (Some(a_s), Some(b_s)) => b_s.cmp(&a_s),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.bitrate.cmp(&a.bitrate),
    });
}

/// Filter formats by codec
pub fn filter_formats_by_codec<'a>(formats: &'a [Format], codec: &str) -> Vec<&'a Format> {
    formats
        .iter()
        .filter(|f| {
            f.audio_codec
                .as_ref()
                .map(|c| c.contains(codec))
                .unwrap_or(false)
                || f.video_codec
                    .as_ref()
                    .map(|c| c.contains(codec))
                    .unwrap_or(false)
        })
        .collect()
}

/// Get format statistics
pub fn get_format_stats(formats: &[Format]) -> FormatStats {
    let mut stats = FormatStats::default();

    for format in formats {
        stats.total_formats += 1;
        stats.total_bitrate += format.bitrate;

        if let Some(size) = format.size {
            stats.total_size += size;
        }

        if format.is_progressive() {
            stats.progressive_formats += 1;
        }

        if format.is_video_only() {
            stats.video_only_formats += 1;
        }

        if format.is_audio_only() {
            stats.audio_only_formats += 1;
        }

        if let Some(height) = format.height {
            if height > stats.max_height {
                stats.max_height = height;
            }
            if stats.min_height == 0 || height < stats.min_height {
                stats.min_height = height;
            }
        }

        if format.bitrate > stats.max_bitrate {
            stats.max_bitrate = format.bitrate;
        }

        if stats.min_bitrate == 0 || format.bitrate < stats.min_bitrate {
            stats.min_bitrate = format.bitrate;
        }
    }

    if stats.total_formats > 0 {
        stats.avg_bitrate = stats.total_bitrate / stats.total_formats as u32;
    }

    stats
}

/// Format statistics
#[derive(Debug, Default)]
pub struct FormatStats {
    pub total_formats: usize,
    pub progressive_formats: usize,
    pub video_only_formats: usize,
    pub audio_only_formats: usize,
    pub total_bitrate: u32,
    pub avg_bitrate: u32,
    pub max_bitrate: u32,
    pub min_bitrate: u32,
    pub total_size: u64,
    pub max_height: u32,
    pub min_height: u32,
}

impl FormatStats {
    /// Get human-readable total size
    pub fn total_size_string(&self) -> String {
        crate::core::progress::format_bytes(self.total_size)
    }

    /// Get human-readable average bitrate
    pub fn avg_bitrate_string(&self) -> String {
        if self.avg_bitrate > 0 {
            format!("{} kbps", self.avg_bitrate / 1000)
        } else {
            "Unknown".to_string()
        }
    }

    /// Get human-readable max bitrate
    pub fn max_bitrate_string(&self) -> String {
        if self.max_bitrate > 0 {
            format!("{} kbps", self.max_bitrate / 1000)
        } else {
            "Unknown".to_string()
        }
    }

    /// Get human-readable min bitrate
    pub fn min_bitrate_string(&self) -> String {
        if self.min_bitrate > 0 {
            format!("{} kbps", self.min_bitrate / 1000)
        } else {
            "Unknown".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::video_info::Format;

    fn create_test_formats() -> Vec<Format> {
        vec![
            Format {
                itag: 22,
                url: "http://example.com/22".to_string(),
                quality: "720p".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate: 2000000,
                size: Some(100000000),
                signature_cipher: None,
                audio_codec: Some("aac".to_string()),
                video_codec: Some("avc1".to_string()),
                fps: Some(30),
                width: Some(1280),
                height: Some(720),
                audio_sample_rate: Some(44100),
                audio_channels: Some(2),
                language: None,
                note: None,
            },
            Format {
                itag: 18,
                url: "http://example.com/18".to_string(),
                quality: "360p".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate: 1000000,
                size: Some(50000000),
                signature_cipher: None,
                audio_codec: Some("aac".to_string()),
                video_codec: Some("avc1".to_string()),
                fps: Some(30),
                width: Some(640),
                height: Some(360),
                audio_sample_rate: Some(44100),
                audio_channels: Some(2),
                language: None,
                note: None,
            },
            Format {
                itag: 137,
                url: "http://example.com/137".to_string(),
                quality: "1080p".to_string(),
                mime_type: "video/mp4".to_string(),
                bitrate: 5000000,
                size: Some(200000000),
                signature_cipher: None,
                audio_codec: None,
                video_codec: Some("avc1".to_string()),
                fps: Some(30),
                width: Some(1920),
                height: Some(1080),
                audio_sample_rate: None,
                audio_channels: None,
                language: None,
                note: None,
            },
        ]
    }

    #[test]
    fn test_select_format_best() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Best);

        let selected = select_format(&formats, &selector).unwrap();
        assert_eq!(selected.itag, 22); // Best progressive format
    }

    #[test]
    fn test_select_format_worst() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Worst);

        let selected = select_format(&formats, &selector).unwrap();
        assert_eq!(selected.itag, 18); // Worst progressive format
    }

    #[test]
    fn test_select_format_itag() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Itag(137));

        let selected = select_format(&formats, &selector).unwrap();
        assert_eq!(selected.itag, 137);
    }

    #[test]
    fn test_select_format_height_limit() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Best).with_height_limit(720);

        let selected = select_format(&formats, &selector).unwrap();
        assert!(selected.height.unwrap_or(0) <= 720);
    }

    #[test]
    fn test_get_best_progressive_format() {
        let formats = create_test_formats();
        let best = get_best_progressive_format(&formats).unwrap();
        assert_eq!(best.itag, 22);
    }

    #[test]
    fn test_get_best_video_format() {
        let formats = create_test_formats();
        let best = get_best_video_format(&formats).unwrap();
        assert_eq!(best.itag, 137);
    }

    #[test]
    fn test_get_format_stats() {
        let formats = create_test_formats();
        let stats = get_format_stats(&formats);

        assert_eq!(stats.total_formats, 3);
        assert_eq!(stats.progressive_formats, 2);
        assert_eq!(stats.video_only_formats, 1);
        assert_eq!(stats.audio_only_formats, 0);
        assert_eq!(stats.max_height, 1080);
        assert_eq!(stats.min_height, 360);
        assert_eq!(stats.max_bitrate, 5000000);
        assert_eq!(stats.min_bitrate, 1000000);
    }

    #[test]
    fn test_select_format_height() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Height(720));

        let selected = select_format(&formats, &selector).unwrap();
        assert_eq!(selected.itag, 22);
        assert_eq!(selected.height, Some(720));
    }

    #[test]
    fn test_select_format_height_le() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::HeightLessOrEqual(720));

        let selected = select_format(&formats, &selector).unwrap();
        assert!(selected.height.unwrap_or(0) <= 720);
    }

    #[test]
    fn test_select_format_height_ge() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::HeightGreaterOrEqual(720));

        let selected = select_format(&formats, &selector).unwrap();
        assert!(selected.height.unwrap_or(0) >= 720);
    }

    #[test]
    fn test_select_format_with_extension() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Best).with_extension("mp4");

        let selected = select_format(&formats, &selector).unwrap();
        assert!(selected.mime_type.contains("mp4"));
    }

    #[test]
    fn test_select_format_with_height_min() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Best).with_height_min(720);

        let selected = select_format(&formats, &selector).unwrap();
        assert!(selected.height.unwrap_or(0) >= 720);
    }

    #[test]
    fn test_select_format_with_preferred_itag() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Best).with_itag(18);

        let selected = select_format(&formats, &selector).unwrap();
        assert_eq!(selected.itag, 18);
    }

    #[test]
    fn test_select_format_no_candidates() {
        let formats = create_test_formats();
        let selector = FormatSelector::new(QualitySelector::Itag(999));

        let result = select_format(&formats, &selector);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RytError::NoFormatFound));
    }

    #[test]
    fn test_get_best_audio_format() {
        let mut formats = create_test_formats();
        // Add an audio-only format
        formats.push(Format {
            itag: 140,
            url: "http://example.com/140".to_string(),
            quality: "audio".to_string(),
            mime_type: "audio/mp4".to_string(),
            bitrate: 128000,
            size: Some(10000000),
            signature_cipher: None,
            audio_codec: Some("aac".to_string()),
            video_codec: None,
            fps: None,
            width: None,
            height: None,
            audio_sample_rate: Some(44100),
            audio_channels: Some(2),
            language: None,
            note: None,
        });

        let best = get_best_audio_format(&formats).unwrap();
        assert_eq!(best.itag, 140);
    }

    #[test]
    fn test_get_formats_by_container() {
        let formats = create_test_formats();
        let mp4_formats = get_formats_by_container(&formats, "mp4");
        assert_eq!(mp4_formats.len(), 3);
    }

    #[test]
    fn test_get_formats_by_quality() {
        let formats = create_test_formats();
        let hd_formats = get_formats_by_quality(&formats, "720p");
        assert_eq!(hd_formats.len(), 1);
        assert_eq!(hd_formats[0].itag, 22);
    }

    #[test]
    fn test_get_formats_by_height_range() {
        let formats = create_test_formats();
        let medium_formats = get_formats_by_height_range(&formats, 400, 800);
        assert_eq!(medium_formats.len(), 1);
        assert_eq!(medium_formats[0].itag, 22);
    }

    #[test]
    fn test_get_formats_by_bitrate_range() {
        let formats = create_test_formats();
        let medium_bitrate_formats = get_formats_by_bitrate_range(&formats, 1000000, 3000000);
        assert_eq!(medium_bitrate_formats.len(), 2);
    }

    #[test]
    fn test_sort_formats_by_quality() {
        let mut formats = create_test_formats();
        sort_formats_by_quality(&mut formats);

        // Should be sorted by height (descending)
        assert_eq!(formats[0].itag, 137); // 1080p
        assert_eq!(formats[1].itag, 22); // 720p
        assert_eq!(formats[2].itag, 18); // 360p
    }

    #[test]
    fn test_sort_formats_by_bitrate() {
        let mut formats = create_test_formats();
        sort_formats_by_bitrate(&mut formats);

        // Should be sorted by bitrate (descending)
        assert_eq!(formats[0].itag, 137); // 5000000
        assert_eq!(formats[1].itag, 22); // 2000000
        assert_eq!(formats[2].itag, 18); // 1000000
    }

    #[test]
    fn test_sort_formats_by_size() {
        let mut formats = create_test_formats();
        sort_formats_by_size(&mut formats);

        // Should be sorted by size (descending)
        assert_eq!(formats[0].itag, 137); // 200000000
        assert_eq!(formats[1].itag, 22); // 100000000
        assert_eq!(formats[2].itag, 18); // 50000000
    }

    #[test]
    fn test_filter_formats_by_codec() {
        let formats = create_test_formats();
        let avc1_formats = filter_formats_by_codec(&formats, "avc1");
        assert_eq!(avc1_formats.len(), 3);
    }

    #[test]
    fn test_filter_formats_by_audio_codec() {
        let formats = create_test_formats();
        let aac_formats = filter_formats_by_codec(&formats, "aac");
        assert_eq!(aac_formats.len(), 2); // Only progressive formats have audio
    }

    #[test]
    fn test_format_stats_string_methods() {
        let formats = create_test_formats();
        let stats = get_format_stats(&formats);

        // Test string formatting methods
        assert!(!stats.total_size_string().is_empty());
        assert!(!stats.avg_bitrate_string().is_empty());
        assert!(!stats.max_bitrate_string().is_empty());
        assert!(!stats.min_bitrate_string().is_empty());

        // Test specific values
        assert_eq!(stats.avg_bitrate_string(), "2666 kbps"); // (2000000 + 1000000 + 5000000) / 3 / 1000
        assert_eq!(stats.max_bitrate_string(), "5000 kbps");
        assert_eq!(stats.min_bitrate_string(), "1000 kbps");
    }

    #[test]
    fn test_format_stats_empty_formats() {
        let formats = vec![];
        let stats = get_format_stats(&formats);

        assert_eq!(stats.total_formats, 0);
        assert_eq!(stats.progressive_formats, 0);
        assert_eq!(stats.video_only_formats, 0);
        assert_eq!(stats.audio_only_formats, 0);
        assert_eq!(stats.total_bitrate, 0);
        assert_eq!(stats.avg_bitrate, 0);
        assert_eq!(stats.max_bitrate, 0);
        assert_eq!(stats.min_bitrate, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.max_height, 0);
        assert_eq!(stats.min_height, 0);
    }

    #[test]
    fn test_format_stats_zero_bitrate_strings() {
        let stats = FormatStats {
            total_formats: 0,
            progressive_formats: 0,
            video_only_formats: 0,
            audio_only_formats: 0,
            total_bitrate: 0,
            avg_bitrate: 0,
            max_bitrate: 0,
            min_bitrate: 0,
            total_size: 0,
            max_height: 0,
            min_height: 0,
        };

        assert_eq!(stats.avg_bitrate_string(), "Unknown");
        assert_eq!(stats.max_bitrate_string(), "Unknown");
        assert_eq!(stats.min_bitrate_string(), "Unknown");
    }

    #[test]
    fn test_format_stats_default() {
        let stats = FormatStats::default();
        assert_eq!(stats.total_formats, 0);
        assert_eq!(stats.progressive_formats, 0);
        assert_eq!(stats.video_only_formats, 0);
        assert_eq!(stats.audio_only_formats, 0);
        assert_eq!(stats.total_bitrate, 0);
        assert_eq!(stats.avg_bitrate, 0);
        assert_eq!(stats.max_bitrate, 0);
        assert_eq!(stats.min_bitrate, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.max_height, 0);
        assert_eq!(stats.min_height, 0);
    }

    #[test]
    fn test_select_format_edge_cases() {
        // Test with empty formats
        let empty_formats = vec![];
        let selector = FormatSelector::new(QualitySelector::Best);
        let result = select_format(&empty_formats, &selector);
        assert!(result.is_err());

        // Test with formats that have no height
        let no_height_formats = vec![Format {
            itag: 999,
            url: "http://example.com/999".to_string(),
            quality: "unknown".to_string(),
            mime_type: "video/mp4".to_string(),
            bitrate: 1000000,
            size: Some(1000000),
            signature_cipher: None,
            audio_codec: None,
            video_codec: None,
            fps: None,
            width: None,
            height: None,
            audio_sample_rate: None,
            audio_channels: None,
            language: None,
            note: None,
        }];

        let selector = FormatSelector::new(QualitySelector::Height(720));
        let result = select_format(&no_height_formats, &selector);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_best_progressive_format_empty() {
        let formats = vec![];
        let result = get_best_progressive_format(&formats);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_best_video_format_empty() {
        let formats = vec![];
        let result = get_best_video_format(&formats);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_best_audio_format_empty() {
        let formats = vec![];
        let result = get_best_audio_format(&formats);
        assert!(result.is_none());
    }
}
