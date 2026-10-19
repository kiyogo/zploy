use zploy::config::UploadConfig;
use zploy::security::path_traversal::{resolve_safe_path, PathSecurityError};
use zploy::security::zip_security::{safe_extract_zip, ZipSecurityError};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[test]
fn test_path_traversal_attacks() {
    let dir = tempdir().unwrap();
    let base = dir.path();

    // 1. Direct parent directory traversal
    assert_eq!(
        resolve_safe_path(base, "../etc/passwd"),
        Err(PathSecurityError::TraversalAttempt)
    );

    // 2. Nested parent directory traversal
    assert_eq!(
        resolve_safe_path(base, "assets/../../secret.txt"),
        Err(PathSecurityError::TraversalAttempt)
    );

    // 3. Windows backslash traversal
    assert_eq!(
        resolve_safe_path(base, "assets\\..\\..\\win.ini"),
        Err(PathSecurityError::TraversalAttempt)
    );

    // 4. Null byte injection
    assert_eq!(
        resolve_safe_path(base, "index.html\0.exe"),
        Err(PathSecurityError::NullByte)
    );

    // 5. Normal safe relative paths must succeed
    assert!(resolve_safe_path(base, "index.html").is_ok());
    assert!(resolve_safe_path(base, "assets/css/style.css").is_ok());
}

#[test]
fn test_zip_slip_attack_rejected() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("malicious_slip.zip");
    let target_extract_dir = dir.path().join("extracted");
    std::fs::create_dir_all(&target_extract_dir).unwrap();

    // Create a zip with a traversal entry
    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file("../../script.txt", options).unwrap();
        zip.write_all(b"sample content").unwrap();
        zip.finish().unwrap();
    }

    let config = UploadConfig {
        max_upload_size_mb: 10,
        max_uncompressed_size_mb: 20,
        max_file_count: 50,
        max_single_file_size_mb: 5,
        allowed_extensions: vec!["html".into(), "css".into(), "txt".into()],
    };

    let result = safe_extract_zip(&zip_path, &target_extract_dir, &config);
    assert!(result.is_err(), "Zip Slip entry must be rejected!");
}

#[test]
fn test_disallowed_extension_rejection() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("bad_ext.zip");
    let target_extract_dir = dir.path().join("extracted");
    std::fs::create_dir_all(&target_extract_dir).unwrap();

    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file("index.html", options).unwrap();
        zip.write_all(b"<h1>Hello</h1>").unwrap();

        zip.start_file("server_script.py", options).unwrap();
        zip.write_all(b"print('hello server')").unwrap();

        zip.finish().unwrap();
    }

    let config = UploadConfig {
        max_upload_size_mb: 10,
        max_uncompressed_size_mb: 20,
        max_file_count: 50,
        max_single_file_size_mb: 5,
        allowed_extensions: vec!["html".into(), "css".into(), "js".into()],
    };

    let result = safe_extract_zip(&zip_path, &target_extract_dir, &config);
    match result {
        Err(ZipSecurityError::DisallowedExtension(file)) => {
            assert!(file.contains("server_script.py"));
        }
        _ => panic!("Expected DisallowedExtension error, got {:?}", result.err()),
    }
}

#[test]
fn test_zip_missing_index_html() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("no_index.zip");
    let target_extract_dir = dir.path().join("extracted");
    std::fs::create_dir_all(&target_extract_dir).unwrap();

    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file("style.css", options).unwrap();
        zip.write_all(b"body { color: red; }").unwrap();

        zip.finish().unwrap();
    }

    let config = UploadConfig {
        max_upload_size_mb: 10,
        max_uncompressed_size_mb: 20,
        max_file_count: 50,
        max_single_file_size_mb: 5,
        allowed_extensions: vec!["html".into(), "css".into(), "js".into()],
    };

    let result = safe_extract_zip(&zip_path, &target_extract_dir, &config);
    match result {
        Err(ZipSecurityError::MissingIndexHtml) => {}
        _ => panic!("Expected MissingIndexHtml error, got {:?}", result.err()),
    }
}

#[test]
fn test_zip_single_root_folder_flattening() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("wrapped_site.zip");
    let target_extract_dir = dir.path().join("extracted");
    std::fs::create_dir_all(&target_extract_dir).unwrap();

    // Site is inside a single folder 'my-portfolio/'
    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        zip.start_file("my-portfolio/index.html", options).unwrap();
        zip.write_all(b"<!DOCTYPE html><html><body><h1>Portfolio</h1></body></html>").unwrap();

        zip.start_file("my-portfolio/css/style.css", options).unwrap();
        zip.write_all(b"body { margin: 0; }").unwrap();

        zip.finish().unwrap();
    }

    let config = UploadConfig {
        max_upload_size_mb: 10,
        max_uncompressed_size_mb: 20,
        max_file_count: 50,
        max_single_file_size_mb: 5,
        allowed_extensions: vec!["html".into(), "css".into(), "js".into()],
    };

    let result = safe_extract_zip(&zip_path, &target_extract_dir, &config).unwrap();
    assert_eq!(result.file_count, 2);
    assert_eq!(result.has_index_html, true);
    assert_eq!(result.detected_root_prefix, Some("my-portfolio/".to_string()));

    // index.html MUST now exist directly at target_extract_dir/index.html
    assert!(target_extract_dir.join("index.html").is_file());
    assert!(target_extract_dir.join("css/style.css").is_file());
}
