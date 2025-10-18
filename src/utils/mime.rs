//! MIME type utilities for determining file extensions

/// Get file extension from MIME type
pub fn ext_from_mime(mime_type: &str) -> &'static str {
    match mime_type {
        // Video formats
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/3gpp" => "3gp",
        "video/x-flv" => "flv",
        "video/quicktime" => "mov",
        "video/x-msvideo" => "avi",
        "video/x-ms-wmv" => "wmv",
        "video/mp2t" => "ts",
        "video/mp2p" => "mpeg",
        "video/mpeg" => "mpeg",
        "video/ogg" => "ogv",
        "video/x-matroska" => "mkv",

        // Audio formats
        "audio/mp4" => "m4a",
        "audio/webm" => "webm",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        "audio/aac" => "aac",
        "audio/x-aac" => "aac",
        "audio/vorbis" => "ogg",
        "audio/opus" => "opus",

        // Default fallback
        _ => "bin",
    }
}

/// Get MIME type from file extension
pub fn mime_from_ext(extension: &str) -> &'static str {
    let ext = extension.trim_start_matches('.').to_lowercase();
    match ext.as_str() {
        // Video formats
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "3gp" => "video/3gpp",
        "flv" => "video/x-flv",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "ts" => "video/mp2t",
        "mpeg" | "mpg" => "video/mpeg",
        "ogv" => "video/ogg",
        "mkv" => "video/x-matroska",

        // Audio formats
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "opus" => "audio/opus",

        // Default fallback
        _ => "application/octet-stream",
    }
}

/// Check if MIME type is a video format
pub fn is_video_mime(mime_type: &str) -> bool {
    mime_type.starts_with("video/")
}

/// Check if MIME type is an audio format
pub fn is_audio_mime(mime_type: &str) -> bool {
    mime_type.starts_with("audio/")
}

/// Check if MIME type is a progressive format (video+audio combined)
pub fn is_progressive_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "video/mp4" | "video/webm" | "video/3gpp" | "video/x-flv"
    )
}

/// Check if MIME type is an adaptive format (video or audio only)
pub fn is_adaptive_mime(mime_type: &str) -> bool {
    is_video_mime(mime_type) || is_audio_mime(mime_type)
}

/// Get container format from MIME type
pub fn get_container_format(mime_type: &str) -> &'static str {
    match mime_type {
        "video/mp4" | "audio/mp4" => "mp4",
        "video/webm" | "audio/webm" => "webm",
        "video/3gpp" => "3gp",
        "video/x-flv" => "flv",
        "video/quicktime" => "mov",
        "video/x-msvideo" => "avi",
        "video/x-ms-wmv" => "wmv",
        "video/mp2t" => "ts",
        "video/mpeg" => "mpeg",
        "video/ogg" | "audio/ogg" => "ogg",
        "video/x-matroska" => "mkv",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        "audio/aac" | "audio/x-aac" => "aac",
        "audio/opus" => "opus",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext_from_mime() {
        assert_eq!(ext_from_mime("video/mp4"), "mp4");
        assert_eq!(ext_from_mime("video/webm"), "webm");
        assert_eq!(ext_from_mime("audio/mp4"), "m4a");
        assert_eq!(ext_from_mime("audio/mpeg"), "mp3");
        assert_eq!(ext_from_mime("unknown/type"), "bin");
    }

    #[test]
    fn test_mime_from_ext() {
        assert_eq!(mime_from_ext("mp4"), "video/mp4");
        assert_eq!(mime_from_ext(".mp4"), "video/mp4");
        assert_eq!(mime_from_ext("MP4"), "video/mp4");
        assert_eq!(mime_from_ext("m4a"), "audio/mp4");
        assert_eq!(mime_from_ext("mp3"), "audio/mpeg");
        assert_eq!(mime_from_ext("unknown"), "application/octet-stream");
    }

    #[test]
    fn test_is_video_mime() {
        assert!(is_video_mime("video/mp4"));
        assert!(is_video_mime("video/webm"));
        assert!(!is_video_mime("audio/mp4"));
        assert!(!is_video_mime("text/plain"));
    }

    #[test]
    fn test_is_audio_mime() {
        assert!(is_audio_mime("audio/mp4"));
        assert!(is_audio_mime("audio/mpeg"));
        assert!(!is_audio_mime("video/mp4"));
        assert!(!is_audio_mime("text/plain"));
    }

    #[test]
    fn test_is_progressive_mime() {
        assert!(is_progressive_mime("video/mp4"));
        assert!(is_progressive_mime("video/webm"));
        assert!(!is_progressive_mime("video/quicktime"));
        assert!(!is_progressive_mime("audio/mp4"));
    }

    #[test]
    fn test_get_container_format() {
        assert_eq!(get_container_format("video/mp4"), "mp4");
        assert_eq!(get_container_format("audio/mp4"), "mp4");
        assert_eq!(get_container_format("video/webm"), "webm");
        assert_eq!(get_container_format("audio/mpeg"), "mp3");
        assert_eq!(get_container_format("unknown/type"), "unknown");
    }

    #[test]
    fn test_is_adaptive_mime() {
        assert!(is_adaptive_mime("video/mp4"));
        assert!(is_adaptive_mime("audio/mp4"));
        assert!(is_adaptive_mime("video/webm"));
        assert!(is_adaptive_mime("audio/mpeg"));
        assert!(!is_adaptive_mime("text/plain"));
        assert!(!is_adaptive_mime("application/json"));
    }

    #[test]
    fn test_ext_from_mime_comprehensive() {
        // Test all video formats
        assert_eq!(ext_from_mime("video/mp4"), "mp4");
        assert_eq!(ext_from_mime("video/webm"), "webm");
        assert_eq!(ext_from_mime("video/3gpp"), "3gp");
        assert_eq!(ext_from_mime("video/x-flv"), "flv");
        assert_eq!(ext_from_mime("video/quicktime"), "mov");
        assert_eq!(ext_from_mime("video/x-msvideo"), "avi");
        assert_eq!(ext_from_mime("video/x-ms-wmv"), "wmv");
        assert_eq!(ext_from_mime("video/mp2t"), "ts");
        assert_eq!(ext_from_mime("video/mp2p"), "mpeg");
        assert_eq!(ext_from_mime("video/mpeg"), "mpeg");
        assert_eq!(ext_from_mime("video/ogg"), "ogv");
        assert_eq!(ext_from_mime("video/x-matroska"), "mkv");

        // Test all audio formats
        assert_eq!(ext_from_mime("audio/mp4"), "m4a");
        assert_eq!(ext_from_mime("audio/webm"), "webm");
        assert_eq!(ext_from_mime("audio/mpeg"), "mp3");
        assert_eq!(ext_from_mime("audio/ogg"), "ogg");
        assert_eq!(ext_from_mime("audio/wav"), "wav");
        assert_eq!(ext_from_mime("audio/x-wav"), "wav");
        assert_eq!(ext_from_mime("audio/flac"), "flac");
        assert_eq!(ext_from_mime("audio/aac"), "aac");
        assert_eq!(ext_from_mime("audio/x-aac"), "aac");
        assert_eq!(ext_from_mime("audio/vorbis"), "ogg");
        assert_eq!(ext_from_mime("audio/opus"), "opus");

        // Test fallback
        assert_eq!(ext_from_mime("unknown/type"), "bin");
    }

    #[test]
    fn test_mime_from_ext_comprehensive() {
        // Test all video formats
        assert_eq!(mime_from_ext("mp4"), "video/mp4");
        assert_eq!(mime_from_ext("webm"), "video/webm");
        assert_eq!(mime_from_ext("3gp"), "video/3gpp");
        assert_eq!(mime_from_ext("flv"), "video/x-flv");
        assert_eq!(mime_from_ext("mov"), "video/quicktime");
        assert_eq!(mime_from_ext("avi"), "video/x-msvideo");
        assert_eq!(mime_from_ext("wmv"), "video/x-ms-wmv");
        assert_eq!(mime_from_ext("ts"), "video/mp2t");
        assert_eq!(mime_from_ext("mpeg"), "video/mpeg");
        assert_eq!(mime_from_ext("mpg"), "video/mpeg");
        assert_eq!(mime_from_ext("ogv"), "video/ogg");
        assert_eq!(mime_from_ext("mkv"), "video/x-matroska");

        // Test all audio formats
        assert_eq!(mime_from_ext("m4a"), "audio/mp4");
        assert_eq!(mime_from_ext("mp3"), "audio/mpeg");
        assert_eq!(mime_from_ext("ogg"), "audio/ogg");
        assert_eq!(mime_from_ext("wav"), "audio/wav");
        assert_eq!(mime_from_ext("flac"), "audio/flac");
        assert_eq!(mime_from_ext("aac"), "audio/aac");
        assert_eq!(mime_from_ext("opus"), "audio/opus");

        // Test with dots and case sensitivity
        assert_eq!(mime_from_ext(".mp4"), "video/mp4");
        assert_eq!(mime_from_ext("MP4"), "video/mp4");
        assert_eq!(mime_from_ext(".MP4"), "video/mp4");

        // Test fallback
        assert_eq!(mime_from_ext("unknown"), "application/octet-stream");
    }

    #[test]
    fn test_get_container_format_comprehensive() {
        // Test all video formats
        assert_eq!(get_container_format("video/mp4"), "mp4");
        assert_eq!(get_container_format("video/webm"), "webm");
        assert_eq!(get_container_format("video/3gpp"), "3gp");
        assert_eq!(get_container_format("video/x-flv"), "flv");
        assert_eq!(get_container_format("video/quicktime"), "mov");
        assert_eq!(get_container_format("video/x-msvideo"), "avi");
        assert_eq!(get_container_format("video/x-ms-wmv"), "wmv");
        assert_eq!(get_container_format("video/mp2t"), "ts");
        assert_eq!(get_container_format("video/mpeg"), "mpeg");
        assert_eq!(get_container_format("video/ogg"), "ogg");
        assert_eq!(get_container_format("video/x-matroska"), "mkv");

        // Test all audio formats
        assert_eq!(get_container_format("audio/mp4"), "mp4");
        assert_eq!(get_container_format("audio/webm"), "webm");
        assert_eq!(get_container_format("audio/mpeg"), "mp3");
        assert_eq!(get_container_format("audio/ogg"), "ogg");
        assert_eq!(get_container_format("audio/wav"), "wav");
        assert_eq!(get_container_format("audio/x-wav"), "wav");
        assert_eq!(get_container_format("audio/flac"), "flac");
        assert_eq!(get_container_format("audio/aac"), "aac");
        assert_eq!(get_container_format("audio/x-aac"), "aac");
        assert_eq!(get_container_format("audio/opus"), "opus");

        // Test fallback
        assert_eq!(get_container_format("unknown/type"), "unknown");
    }
}
