use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use sqlx::MySqlPool;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tracing::info;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::config::Config;
use crate::database::models::{Release, Site};
use crate::security::{normalize_slug, validate_slug};

pub struct SiteService;

impl SiteService {
    pub async fn create_site(
        pool: &MySqlPool,
        owner_id: &str,
        name: &str,
        slug: &str,
        config: &Config,
    ) -> Result<Site> {
        let clean_slug = normalize_slug(slug);
        validate_slug(&clean_slug, &config.security.reserved_slugs)
            .map_err(|e| anyhow!(e.to_string()))?;

        // Check uniqueness
        let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM sites WHERE slug = ? LIMIT 1")
            .bind(&clean_slug)
            .fetch_optional(pool)
            .await?;

        if existing.is_some() {
            if clean_slug.is_empty() {
                return Err(anyhow!("The root path (/) is already in use by another website. Only 1 site is allowed at root."));
            } else {
                return Err(anyhow!("Path '/{}' is already in use by another website", clean_slug));
            }
        }

        let site_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO sites (id, owner_id, name, slug, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'published', ?, ?)
            "#,
        )
        .bind(&site_id)
        .bind(owner_id)
        .bind(name)
        .bind(&clean_slug)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = ?")
            .bind(&site_id)
            .fetch_one(pool)
            .await?;

        Ok(site)
    }

    pub async fn deploy_release(
        pool: &MySqlPool,
        site_id: &str,
        staging_dir: &Path,
        file_count: usize,
        total_size_bytes: u64,
        note: Option<&str>,
        config: &Config,
    ) -> Result<Release> {
        // Find current max version
        let current_max_version: (Option<i64>,) = sqlx::query_as(
            "SELECT MAX(version_num) FROM releases WHERE site_id = ?"
        )
        .bind(site_id)
        .fetch_one(pool)
        .await?;

        let next_version = current_max_version.0.unwrap_or(0) + 1;
        let release_id = uuid::Uuid::new_v4().to_string();
        let release_folder_name = format!("release-{:04}", next_version);

        let site_storage_dir = config.storage.sites_dir.join(site_id).join("releases");
        let release_target_dir = site_storage_dir.join(&release_folder_name);

        std::fs::create_dir_all(&release_target_dir)
            .with_context(|| format!("Failed to create release directory: {:?}", release_target_dir))?;

        // Move all files and directories from staging_dir to release_target_dir
        copy_dir_all(staging_dir, &release_target_dir)
            .context("Error copying assets from staging into release directory")?;

        let relative_storage_path = format!("{}/releases/{}", site_id, release_folder_name);
        let now = Utc::now().to_rfc3339();
        let note_str = note.unwrap_or("New release deployment");

        // Transaction to atomically update release and site
        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO releases (id, site_id, version_num, storage_path, file_count, total_size_bytes, status, note, created_at)
            VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?)
            "#,
        )
        .bind(&release_id)
        .bind(site_id)
        .bind(next_version)
        .bind(&relative_storage_path)
        .bind(file_count as i64)
        .bind(total_size_bytes as i64)
        .bind(note_str)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE sites
            SET active_release_id = ?,
                file_count = ?,
                total_size_bytes = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&release_id)
        .bind(file_count as i64)
        .bind(total_size_bytes as i64)
        .bind(&now)
        .bind(site_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        info!(
            "Successfully deployed release v{} for site {} ({} files, {} bytes)",
            next_version, site_id, file_count, total_size_bytes
        );

        let release = sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = ?")
            .bind(&release_id)
            .fetch_one(pool)
            .await?;

        Ok(release)
    }

    pub async fn rollback_release(
        pool: &MySqlPool,
        site_id: &str,
        target_release_id: &str,
    ) -> Result<Site> {
        let release = sqlx::query_as::<_, Release>(
            "SELECT * FROM releases WHERE id = ? AND site_id = ?"
        )
        .bind(target_release_id)
        .bind(site_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Valid release for rollback not found"))?;

        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE sites
            SET active_release_id = ?,
                file_count = ?,
                total_size_bytes = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&release.id)
        .bind(release.file_count)
        .bind(release.total_size_bytes)
        .bind(&now)
        .bind(site_id)
        .execute(pool)
        .await?;

        let updated_site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = ?")
            .bind(site_id)
            .fetch_one(pool)
            .await?;

        Ok(updated_site)
    }

    pub async fn delete_site(
        pool: &MySqlPool,
        site_id: &str,
        config: &Config,
    ) -> Result<()> {
        sqlx::query("DELETE FROM sites WHERE id = ?")
            .bind(site_id)
            .execute(pool)
            .await?;

        let site_dir = config.storage.sites_dir.join(site_id);
        if site_dir.exists() {
            let _ = std::fs::remove_dir_all(&site_dir);
        }

        Ok(())
    }

    pub async fn update_site_metadata(
        pool: &MySqlPool,
        site_id: &str,
        new_name: Option<&str>,
        new_slug: Option<&str>,
        spa_fallback: Option<bool>,
        config: &Config,
    ) -> Result<Site> {
        let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = ?")
            .bind(site_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow!("Website does not exist"))?;

        let updated_name = new_name.map(|n| n.trim()).unwrap_or(&site.name);
        let updated_slug = if let Some(slug) = new_slug {
            let clean = normalize_slug(slug);
            if clean != site.slug {
                validate_slug(&clean, &config.security.reserved_slugs)
                    .map_err(|e| anyhow!(e.to_string()))?;

                let exists: Option<(String,)> = sqlx::query_as(
                    "SELECT id FROM sites WHERE slug = ? AND id != ? LIMIT 1"
                )
                .bind(&clean)
                .bind(site_id)
                .fetch_optional(pool)
                .await?;

                if exists.is_some() {
                    if clean.is_empty() {
                        return Err(anyhow!("The root path (/) is already in use by another website"));
                    } else {
                        return Err(anyhow!("Path '/{}' is already in use", clean));
                    }
                }
            }
            clean
        } else {
            site.slug.clone()
        };

        let updated_spa = spa_fallback.map(|b| if b { 1 } else { 0 }).unwrap_or(site.spa_fallback);
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE sites
            SET name = ?, slug = ?, spa_fallback = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(updated_name)
        .bind(&updated_slug)
        .bind(updated_spa)
        .bind(&now)
        .bind(site_id)
        .execute(pool)
        .await?;

        let updated_site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = ?")
            .bind(site_id)
            .fetch_one(pool)
            .await?;

        Ok(updated_site)
    }

    pub fn list_files_in_release(
        release_abs_dir: &Path,
    ) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();

        if !release_abs_dir.exists() {
            return Ok(files);
        }

        for entry in WalkDir::new(release_abs_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Ok(rel_path) = path.strip_prefix(release_abs_dir) {
                    let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    files.push(FileInfo {
                        path: rel_str,
                        size_bytes: size,
                    });
                }
            }
        }

        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    pub fn create_zip_from_dir(
        source_dir: &Path,
        dest_zip_path: &Path,
    ) -> Result<()> {
        let file = File::create(dest_zip_path)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for entry in WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = match path.strip_prefix(source_dir) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };

            if name.is_empty() {
                continue;
            }

            if entry.file_type().is_file() {
                zip.start_file(&name, options)?;
                let mut f = File::open(path)?;
                let mut buffer = Vec::new();
                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
            } else if entry.file_type().is_dir() {
                zip.add_directory(&format!("{}/", name), options)?;
            }
        }

        zip.finish()?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct FileInfo {
    pub path: String,
    pub size_bytes: u64,
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
