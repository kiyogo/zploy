use anyhow::{anyhow, Context, Result};
use axum::extract::multipart::Multipart;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::config::Config;
use crate::security::path_traversal::resolve_safe_path;
use crate::security::zip_security::safe_extract_zip;

pub struct StagedSiteData {
    pub staging_dir: PathBuf,
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub form_slug: Option<String>,
    pub form_name: Option<String>,
    pub form_note: Option<String>,
}

impl StagedSiteData {
    pub fn cleanup(&self) {
        if self.staging_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.staging_dir);
        }
    }
}

pub async fn process_upload_multipart(
    mut multipart: Multipart,
    config: &Config,
) -> Result<StagedSiteData> {
    let staging_id = uuid::Uuid::new_v4().to_string();
    let staging_dir = config.storage.staging_dir.join(&staging_id);
    std::fs::create_dir_all(&staging_dir)
        .context("Failed to create temporary staging directory")?;

    let mut form_slug: Option<String> = None;
    let mut form_name: Option<String> = None;
    let mut form_note: Option<String> = None;

    let mut total_uploaded_bytes: u64 = 0;
    let max_upload_bytes = (config.upload.max_upload_size_mb as u64) * 1024 * 1024;
    let max_file_bytes = (config.upload.max_single_file_size_mb as u64) * 1024 * 1024;

    let mut zip_temp_file: Option<PathBuf> = None;
    let mut regular_files: Vec<(String, PathBuf)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| anyhow!("Failed to parse multipart upload: {}", e))?
    {
        let name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().map(|f| f.to_string());

        if filename.is_none() || filename.as_deref() == Some("") {
            // Text form field
            let text = field
                .text()
                .await
                .map_err(|e| anyhow!("Failed to read form field: {}", e))?;
            match name.as_str() {
                "slug" => form_slug = Some(text.trim().to_string()),
                "name" => form_name = Some(text.trim().to_string()),
                "note" => form_note = Some(text.trim().to_string()),
                _ => {}
            }
            continue;
        }

        let raw_filename = filename.unwrap();
        let is_zip = raw_filename.to_lowercase().ends_with(".zip");

        if is_zip {
            // Stream ZIP to temp file
            let temp_zip_path = config
                .storage
                .temp_dir
                .join(format!("{}.zip", uuid::Uuid::new_v4()));
            let mut out = File::create(&temp_zip_path)
                .context("Failed to create temporary file for ZIP archive")?;

            let mut field_bytes = field;
            while let Some(chunk) = field_bytes
                .chunk()
                .await
                .map_err(|e| anyhow!("Failed to stream ZIP data: {}", e))?
            {
                total_uploaded_bytes += chunk.len() as u64;
                if total_uploaded_bytes > max_upload_bytes {
                    let _ = std::fs::remove_file(&temp_zip_path);
                    let _ = std::fs::remove_dir_all(&staging_dir);
                    return Err(anyhow!(
                        "Upload size exceeds limit of {} MB",
                        config.upload.max_upload_size_mb
                    ));
                }
                out.write_all(&chunk)?;
            }

            zip_temp_file = Some(temp_zip_path);
            break; // A zip replaces everything
        } else {
            // Regular file
            let clean_name = raw_filename.replace('\\', "/").trim_start_matches('/').to_string();
            let ext = Path::new(&clean_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Check disallowed executable extensions
            let dangerous_exts = ["php", "phtml", "py", "sh", "bash", "exe", "bat", "cmd", "ps1", "vbs", "cgi", "pl", "rb", "jar"];
            if dangerous_exts.contains(&ext.as_str()) {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(anyhow!("File '{}' is not allowed on the server (dangerous file extension)", clean_name));
            }

            let safe_dest = match resolve_safe_path(&staging_dir, &clean_name) {
                Ok(p) => p,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&staging_dir);
                    return Err(anyhow!("File path '{}' is insecure: {:?}", clean_name, e));
                }
            };

            if let Some(parent) = safe_dest.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut out = File::create(&safe_dest)?;
            let mut single_file_bytes: u64 = 0;
            let mut field_bytes = field;

            while let Some(chunk) = field_bytes
                .chunk()
                .await
                .map_err(|e| anyhow!("Failed to stream chunk for file {}: {}", clean_name, e))?
            {
                single_file_bytes += chunk.len() as u64;
                total_uploaded_bytes += chunk.len() as u64;

                if single_file_bytes > max_file_bytes {
                    let _ = std::fs::remove_dir_all(&staging_dir);
                    return Err(anyhow!(
                        "File '{}' exceeds limit of {} MB",
                        clean_name,
                        config.upload.max_single_file_size_mb
                    ));
                }

                if total_uploaded_bytes > max_upload_bytes {
                    let _ = std::fs::remove_dir_all(&staging_dir);
                    return Err(anyhow!(
                        "Total upload size exceeds limit of {} MB",
                        config.upload.max_upload_size_mb
                    ));
                }

                out.write_all(&chunk)?;
            }

            regular_files.push((clean_name, safe_dest));
        }
    }

    // Process extracted or regular files
    if let Some(zip_path) = zip_temp_file {
        let extract_result = safe_extract_zip(&zip_path, &staging_dir, &config.upload);
        let _ = std::fs::remove_file(&zip_path);

        match extract_result {
            Ok(info) => {
                info!(
                    "Safely extracted ZIP archive: {} files, {} bytes",
                    info.file_count, info.total_size_bytes
                );
                Ok(StagedSiteData {
                    staging_dir,
                    file_count: info.file_count,
                    total_size_bytes: info.total_size_bytes,
                    form_slug,
                    form_name,
                    form_note,
                })
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging_dir);
                Err(anyhow!(e.to_string()))
            }
        }
    } else if !regular_files.is_empty() {
        // If single HTML file uploaded, ensure it is available as index.html
        if regular_files.len() == 1 {
            let (ref original_name, ref path) = regular_files[0];
            let ext = Path::new(original_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "html" || ext == "htm" {
                let root_index = staging_dir.join("index.html");
                if path != &root_index {
                    let _ = std::fs::copy(path, &root_index);
                }
            }
        }

        // Verify index.html existence
        let has_index = staging_dir.join("index.html").is_file()
            || staging_dir.join("index.htm").is_file();

        if !has_index {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(anyhow!("Uploaded source files are missing 'index.html' at the root"));
        }

        Ok(StagedSiteData {
            staging_dir,
            file_count: regular_files.len(),
            total_size_bytes: total_uploaded_bytes,
            form_slug,
            form_name,
            form_note,
        })
    } else {
        let _ = std::fs::remove_dir_all(&staging_dir);
        Err(anyhow!("No files were uploaded"))
    }
}
