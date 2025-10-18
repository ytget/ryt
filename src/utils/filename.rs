//! Safe filename generation utilities

use regex::Regex;
use std::path::Path;

/// Convert a title to a safe filename by removing/replacing invalid characters
pub fn to_safe_filename(title: &str, extension: &str) -> String {
    // Remove or replace invalid characters for filenames
    let invalid_chars = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
    let mut safe_title = invalid_chars.replace_all(title, "_").to_string();

    // Remove leading/trailing dots and spaces
    safe_title = safe_title
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();

    // Limit length (Windows has 255 char limit, be conservative)
    if safe_title.len() > 200 {
        safe_title.truncate(200);
        safe_title = safe_title.trim_end().to_string();
    }

    // Ensure it's not empty
    if safe_title.is_empty() {
        safe_title = "video".to_string();
    }

    // Add extension if provided
    if !extension.is_empty() {
        let ext = if extension.starts_with('.') {
            extension.to_string()
        } else {
            format!(".{}", extension)
        };
        format!("{}{}", safe_title, ext)
    } else {
        safe_title
    }
}

/// Check if a filename is safe for the current filesystem
pub fn is_safe_filename(filename: &str) -> bool {
    if filename.is_empty() || filename.len() > 255 {
        return false;
    }

    // Check for invalid characters
    let invalid_chars = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
    if invalid_chars.is_match(filename) {
        return false;
    }

    // Check for reserved names on Windows
    let reserved_names = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if let Some(name_without_ext) = Path::new(filename).file_stem() {
        if let Some(name_str) = name_without_ext.to_str() {
            let upper_name = name_str.to_uppercase();
            if reserved_names.contains(&upper_name.as_str()) {
                return false;
            }
        }
    }

    // Check for leading/trailing dots or spaces
    if filename.starts_with('.')
        || filename.ends_with('.')
        || filename.starts_with(' ')
        || filename.ends_with(' ')
    {
        return false;
    }

    true
}

/// Generate a unique filename by appending a number if the file already exists
pub fn generate_unique_filename(base_path: &Path, filename: &str) -> std::io::Result<String> {
    let mut counter = 1;
    let mut final_filename = filename.to_string();

    while base_path.join(&final_filename).exists() {
        let path = Path::new(filename);
        let stem = path.file_stem().unwrap_or_default();
        let extension = path
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy()))
            .unwrap_or_default();

        final_filename = format!("{} ({}){}", stem.to_string_lossy(), counter, extension);
        counter += 1;

        // Prevent infinite loop
        if counter > 10000 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Too many files with similar names",
            ));
        }
    }

    Ok(final_filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_safe_filename() {
        assert_eq!(
            to_safe_filename("Test Video: Title", "mp4"),
            "Test Video_ Title.mp4"
        );

        assert_eq!(
            to_safe_filename("Video with <invalid> chars", "mp4"),
            "Video with _invalid_ chars.mp4"
        );

        assert_eq!(to_safe_filename("", "mp4"), "video.mp4");

        assert_eq!(
            to_safe_filename("Very long title that exceeds the maximum length limit and should be truncated to prevent filesystem issues", "mp4"),
            "Very long title that exceeds the maximum length limit and should be truncated to prevent filesystem issues.mp4"
        );
    }

    #[test]
    fn test_is_safe_filename() {
        assert!(is_safe_filename("normal_file.mp4"));
        assert!(is_safe_filename("video with spaces.mp4"));
        assert!(!is_safe_filename("file<with>invalid:chars.mp4"));
        assert!(!is_safe_filename(""));
        assert!(!is_safe_filename(".hidden_file.mp4"));
        // Note: "file with trailing space .mp4" is actually safe because the space is before the dot, not at the end
        assert!(is_safe_filename("file with trailing space .mp4"));
        assert!(!is_safe_filename("file with trailing space .mp4 "));
    }

    #[test]
    fn test_generate_unique_filename() {
        let temp_dir = std::env::temp_dir();
        let base_path = &temp_dir;

        // This test might fail if the temp directory has existing files
        // In a real scenario, we'd use a dedicated test directory
        let result = generate_unique_filename(base_path, "test_file.mp4");
        assert!(result.is_ok());
    }

    #[test]
    fn test_to_safe_filename_edge_cases() {
        // Test with empty extension
        assert_eq!(to_safe_filename("Test Video", ""), "Test Video");

        // Test with extension starting with dot
        assert_eq!(to_safe_filename("Test Video", ".mp4"), "Test Video.mp4");

        // Test with extension not starting with dot
        assert_eq!(to_safe_filename("Test Video", "mp4"), "Test Video.mp4");

        // Test with very long title (should be truncated)
        let long_title = "a".repeat(250);
        let result = to_safe_filename(&long_title, "mp4");
        assert!(result.len() <= 204); // 200 chars + ".mp4"

        // Test with title that becomes empty after cleaning
        assert_eq!(to_safe_filename("...", "mp4"), "video.mp4");
        assert_eq!(to_safe_filename("   ", "mp4"), "video.mp4");

        // Test with title that has leading/trailing dots and spaces
        assert_eq!(
            to_safe_filename("  .Test Video.  ", "mp4"),
            "Test Video.mp4"
        );
    }

    #[test]
    fn test_is_safe_filename_reserved_names() {
        // Test Windows reserved names
        assert!(!is_safe_filename("CON.mp4"));
        assert!(!is_safe_filename("PRN.mp4"));
        assert!(!is_safe_filename("AUX.mp4"));
        assert!(!is_safe_filename("NUL.mp4"));
        assert!(!is_safe_filename("COM1.mp4"));
        assert!(!is_safe_filename("LPT1.mp4"));

        // Test case insensitive
        assert!(!is_safe_filename("con.mp4"));
        assert!(!is_safe_filename("Con.mp4"));

        // Test with extension
        assert!(!is_safe_filename("CON"));
        assert!(!is_safe_filename("PRN"));
    }

    #[test]
    fn test_is_safe_filename_length_limits() {
        // Test empty filename
        assert!(!is_safe_filename(""));

        // Test too long filename
        let long_filename = "a".repeat(256);
        assert!(!is_safe_filename(&long_filename));

        // Test exactly 255 characters (should be valid)
        let max_filename = "a".repeat(255);
        assert!(is_safe_filename(&max_filename));
    }

    #[test]
    fn test_is_safe_filename_invalid_chars() {
        // Test various invalid characters
        assert!(!is_safe_filename("file<test>.mp4"));
        assert!(!is_safe_filename("file>test.mp4"));
        assert!(!is_safe_filename("file:test.mp4"));
        assert!(!is_safe_filename("file\"test.mp4"));
        assert!(!is_safe_filename("file/test.mp4"));
        assert!(!is_safe_filename("file\\test.mp4"));
        assert!(!is_safe_filename("file|test.mp4"));
        assert!(!is_safe_filename("file?test.mp4"));
        assert!(!is_safe_filename("file*test.mp4"));

        // Test control characters
        assert!(!is_safe_filename("file\x00test.mp4"));
        assert!(!is_safe_filename("file\x1ftest.mp4"));
    }

    #[test]
    fn test_is_safe_filename_leading_trailing_chars() {
        // Test leading/trailing dots
        assert!(!is_safe_filename(".file.mp4"));
        assert!(!is_safe_filename("file.mp4."));

        // Test leading/trailing spaces
        assert!(!is_safe_filename(" file.mp4"));
        assert!(!is_safe_filename("file.mp4 "));

        // Test valid cases
        assert!(is_safe_filename("file.mp4"));
        assert!(is_safe_filename("file with spaces.mp4"));
    }

    #[test]
    fn test_generate_unique_filename_edge_cases() {
        use std::fs::File;

        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("ryt_test_unique_filename");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Test with non-existent file
        let result = generate_unique_filename(&test_dir, "nonexistent.mp4");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nonexistent.mp4");

        // Test with existing file
        let existing_file = test_dir.join("existing.mp4");
        File::create(&existing_file).unwrap();

        let result = generate_unique_filename(&test_dir, "existing.mp4");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "existing (1).mp4");

        // Test with file without extension
        let existing_file_no_ext = test_dir.join("existing");
        File::create(&existing_file_no_ext).unwrap();

        let result = generate_unique_filename(&test_dir, "existing");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "existing (1)");

        // Clean up
        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
