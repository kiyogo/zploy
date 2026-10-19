#![allow(dead_code)]
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use thiserror::Error;
use tracing::warn;
use zip::ZipArchive;

use crate::config::UploadConfig;
use crate::security::path_traversal::{resolve_safe_path, PathSecurityError};

#[derive(Debug, Error)]
pub enum ZipSecurityError {
    #[error("ZIP format error: {0}")]
    FormatError(String),
    #[error("Zip Slip path traversal attack detected: {0}")]
    ZipSlip(String),
    #[error("File count exceeds limit (maximum {0} files)")]
    TooManyFiles(usize),
    #[error("File '{0}' ({1} bytes) exceeds single file size limit ({2} MB)")]
    FileTooLarge(String, u64, usize),
    #[error("Total uncompressed size exceeds limit ({0} MB)")]
    TotalSizeTooLarge(usize),
    #[error("File '{0}' has a disallowed or unsupported extension (only static web assets are accepted)")]
    DisallowedExtension(String),
    #[error("No 'index.html' found in uploaded source files")]
    MissingIndexHtml,
    #[error("I/O error during extraction: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Path security error: {0}")]
    PathError(#[from] PathSecurityError),
}

pub struct ExtractedSiteInfo {
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub has_index_html: bool,
    pub detected_root_prefix: Option<String>,
}

/// Safely inspects and extracts a zip file into `target_dir`
pub fn safe_extract_zip(
    zip_path: &Path,
    target_dir: &Path,
    config: &UploadConfig,
) -> Result<ExtractedSiteInfo, ZipSecurityError> {
    let file = File::open(zip_path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| ZipSecurityError::FormatError(e.to_string()))?;

    let total_entries = archive.len();
    if total_entries > config.max_file_count {
        return Err(ZipSecurityError::TooManyFiles(config.max_file_count));
    }

    // Pass 1: Analyze directory structure to see if there is a single common root directory
    let mut non_empty_entries = Vec::new();
    for i in 0..total_entries {
        let entry = archive.by_index(i).map_err(|e| ZipSecurityError::FormatError(e.to_string()))?;
        let name = entry.name().replace('\\', "/");
        if !name.trim().is_empty() && !name.ends_with('/') {
            non_empty_entries.push(name);
        }
    }

    let common_root = detect_common_root_dir(&non_empty_entries);

    let max_total_bytes = (config.max_uncompressed_size_mb as u64) * 1024 * 1024;
    let max_file_bytes = (config.max_single_file_size_mb as u64) * 1024 * 1024;

    let mut current_total_bytes: u64 = 0;
    let mut extracted_file_count: usize = 0;
    let mut has_index_html = false;

    // Pass 2: Extract files with strict limits
    for i in 0..total_entries {
        let mut entry = archive.by_index(i).map_err(|e| ZipSecurityError::FormatError(e.to_string()))?;
        let raw_name = entry.name().replace('\\', "/");

        // Strip leading common root if detected
        let clean_relative_path = if let Some(ref root) = common_root {
            if raw_name.starts_with(root) {
                raw_name[root.len()..].trim_start_matches('/').to_string()
            } else {
                raw_name.clone()
            }
        } else {
            raw_name.trim_start_matches('/').to_string()
        };

        if clean_relative_path.is_empty() {
            continue;
        }

        // Check for directory entry
        if entry.is_dir() || clean_relative_path.ends_with('/') {
            let safe_dir = resolve_safe_path(target_dir, &clean_relative_path)?;
            std::fs::create_dir_all(&safe_dir)?;
            continue;
        }

        // Validate extension
        let path_obj = Path::new(&clean_relative_path);
        let ext = path_obj
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Reject dangerous executable extensions explicitly
        let dangerous_exts = ["php", "phtml", "py", "sh", "bash", "exe", "bat", "cmd", "ps1", "vbs", "cgi", "pl", "rb", "jar"];
        if dangerous_exts.contains(&ext.as_str()) {
            return Err(ZipSecurityError::DisallowedExtension(clean_relative_path));
        }

        // Check if extension is in allowed list (if configured)
        if !ext.is_empty() && !config.allowed_extensions.iter().any(|a| a.eq_ignore_ascii_case(&ext)) {
            warn!("Rejected file with non-whitelisted extension: {}", clean_relative_path);
            return Err(ZipSecurityError::DisallowedExtension(clean_relative_path));
        }

        // Check file size limits
        let declared_size = entry.size();
        if declared_size > max_file_bytes {
            return Err(ZipSecurityError::FileTooLarge(
                clean_relative_path,
                declared_size,
                config.max_single_file_size_mb,
            ));
        }

        // Resolve safe path
        let safe_out_path = resolve_safe_path(target_dir, &clean_relative_path)?;

        if let Some(parent) = safe_out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut out_file = File::create(&safe_out_path)?;
        let mut buffer = [0u8; 8192];
        let mut entry_read_bytes: u64 = 0;

        loop {
            let bytes_read = entry.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            entry_read_bytes += bytes_read as u64;
            current_total_bytes += bytes_read as u64;

            if entry_read_bytes > max_file_bytes {
                return Err(ZipSecurityError::FileTooLarge(
                    clean_relative_path,
                    entry_read_bytes,
                    config.max_single_file_size_mb,
                ));
            }

            if current_total_bytes > max_total_bytes {
                return Err(ZipSecurityError::TotalSizeTooLarge(
                    config.max_uncompressed_size_mb,
                ));
            }

            out_file.write_all(&buffer[..bytes_read])?;
        }

        extracted_file_count += 1;

        // Check if this is index.html at root
        if clean_relative_path.eq_ignore_ascii_case("index.html")
            || clean_relative_path.eq_ignore_ascii_case("index.htm")
        {
            has_index_html = true;
        }
    }

    // Verify index.html existence
    if !has_index_html {
        return Err(ZipSecurityError::MissingIndexHtml);
    }

    Ok(ExtractedSiteInfo {
        file_count: extracted_file_count,
        total_size_bytes: current_total_bytes,
        has_index_html,
        detected_root_prefix: common_root,
    })
}

fn detect_common_root_dir(entries: &[String]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }

    let first = &entries[0];
    let first_slash = first.find('/')?;
    let candidate = &first[..=first_slash];

    for entry in entries {
        if !entry.starts_with(candidate) {
            return None;
        }
    }

    Some(candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_common_root() {
        let entries = vec![
            "my-project/index.html".to_string(),
            "my-project/css/style.css".to_string(),
            "my-project/js/app.js".to_string(),
        ];
        assert_eq!(detect_common_root_dir(&entries), Some("my-project/".to_string()));

        let mixed = vec![
            "index.html".to_string(),
            "css/style.css".to_string(),
        ];
        assert_eq!(detect_common_root_dir(&mixed), None);
    }
}
