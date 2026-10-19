use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PathSecurityError {
    #[error("Path contains invalid NUL byte")]
    NullByte,
    #[error("Path traversal attack detected: path contains '..' or dangerous characters")]
    TraversalAttempt,
    #[error("Absolute paths are not allowed")]
    AbsolutePathNotAllowed,
    #[error("Path points outside the allowed base directory")]
    EscapedBaseDir,
    #[error("Path contains an insecure symbolic link")]
    InsecureSymlink,
}

pub fn resolve_safe_path(base_dir: &Path, relative_subpath: &str) -> Result<PathBuf, PathSecurityError> {
    if relative_subpath.contains('\0') {
        return Err(PathSecurityError::NullByte);
    }

    // Normalize forward and backward slashes
    let sanitized_input = relative_subpath
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();

    let mut safe_path = PathBuf::from(base_dir);

    for component in Path::new(&sanitized_input).components() {
        match component {
            Component::Normal(c) => {
                let s = c.to_string_lossy();
                if s == "." || s == ".." {
                    return Err(PathSecurityError::TraversalAttempt);
                }
                safe_path.push(c);
            }
            Component::CurDir => continue,
            Component::ParentDir => return Err(PathSecurityError::TraversalAttempt),
            Component::RootDir | Component::Prefix(_) => return Err(PathSecurityError::AbsolutePathNotAllowed),
        }
    }

    // Canonicalize check if base_dir exists
    let canonical_base = base_dir.canonicalize().unwrap_or_else(|_| base_dir.to_path_buf());
    
    // If the target file exists, check canonical path
    if safe_path.exists() {
        let symlink_meta = std::fs::symlink_metadata(&safe_path).ok();
        if let Some(meta) = symlink_meta {
            if meta.file_type().is_symlink() {
                // Reject symlinks to prevent symlink escape
                return Err(PathSecurityError::InsecureSymlink);
            }
        }

        let canonical_target = match safe_path.canonicalize() {
            Ok(p) => p,
            Err(_) => return Err(PathSecurityError::EscapedBaseDir),
        };

        if !canonical_target.starts_with(&canonical_base) {
            return Err(PathSecurityError::EscapedBaseDir);
        }
    }

    Ok(safe_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_security_checks() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        // Valid relative paths
        let res = resolve_safe_path(base, "assets/css/style.css");
        assert!(res.is_ok());

        // Traversal attempts
        assert_eq!(
            resolve_safe_path(base, "../etc/passwd"),
            Err(PathSecurityError::TraversalAttempt)
        );
        assert_eq!(
            resolve_safe_path(base, "assets/../../secret.txt"),
            Err(PathSecurityError::TraversalAttempt)
        );
        assert_eq!(
            resolve_safe_path(base, "something/..\\..\\win.ini"),
            Err(PathSecurityError::TraversalAttempt)
        );

        // Null bytes
        assert_eq!(
            resolve_safe_path(base, "index.html\0.exe"),
            Err(PathSecurityError::NullByte)
        );
    }
}
