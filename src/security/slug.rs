use regex::Regex;
use std::sync::LazyLock;
use thiserror::Error;

static SEGMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9](?:[a-z0-9\-_]{0,62}[a-z0-9])?$").expect("Invalid segment regex")
});

#[derive(Debug, Error, PartialEq)]
pub enum SlugValidationError {
    #[error("Path is too long (maximum 128 characters)")]
    TooLong,
    #[error("Path may only contain lowercase letters (a-z), numbers (0-9), hyphens (-), underscores (_), and slashes (/); each segment must start and end with an alphanumeric character")]
    InvalidCharacters,
    #[error("Path contains a reserved system keyword ('{0}'); please choose another path")]
    Reserved(String),
}

/// Normalizes a slug:
/// - Trims leading/trailing whitespace and slashes
/// - Converts to lowercase
/// - Collapses multiple consecutive slashes (e.g. `a//b` -> `a/b`)
/// - Empty string or "/" returns "" (representing the root website /)
pub fn normalize_slug(slug: &str) -> String {
    let trimmed = slug.trim().trim_matches(|c| c == '/' || c == ' ');
    let lower = trimmed.to_lowercase();
    let parts: Vec<&str> = lower.split('/').filter(|p| !p.is_empty()).collect();
    parts.join("/")
}

/// Validates normalized slug:
/// - Empty slug ("") is valid (represents root website /)
/// - Allows forward slashes '/' for path hierarchies (e.g. `20/10`, `shop/clothing`)
/// - Each segment between slashes must adhere to a-z, 0-9, '-', '_'
/// - First segment must not collide with reserved system keywords (such as api, admin, assets)
pub fn validate_slug(slug: &str, reserved_slugs: &[String]) -> Result<(), SlugValidationError> {
    let normalized = normalize_slug(slug);
    if normalized.is_empty() {
        // Root path (/) is allowed
        return Ok(());
    }

    if normalized.len() > 128 {
        return Err(SlugValidationError::TooLong);
    }

    let segments: Vec<&str> = normalized.split('/').collect();
    for seg in &segments {
        if seg.is_empty() || *seg == "." || *seg == ".." || !SEGMENT_REGEX.is_match(seg) {
            return Err(SlugValidationError::InvalidCharacters);
        }
    }

    // Check if first segment collides with reserved system keyword
    if let Some(first_seg) = segments.first() {
        for reserved in reserved_slugs {
            if first_seg.eq_ignore_ascii_case(reserved) {
                return Err(SlugValidationError::Reserved(first_seg.to_string()));
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub fn sanitize_suggested_slug(input: &str) -> String {
    let lower = input.trim().to_lowercase();
    let re = Regex::new(r"[^a-z0-9\-_/]+").unwrap();
    let cleaned = re.replace_all(&lower, "-");
    let normalized = normalize_slug(&cleaned);
    let parts: Vec<&str> = normalized
        .split('/')
        .map(|seg| seg.trim_matches(|c| c == '-' || c == '_'))
        .filter(|seg| !seg.is_empty())
        .collect();
    let joined = parts.join("/");
    if joined.len() > 128 {
        joined[..128].trim_matches(|c| c == '-' || c == '_' || c == '/').to_string()
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_slugs() {
        let reserved = vec!["api".to_string(), "admin".to_string()];
        // Root slug
        assert_eq!(validate_slug("", &reserved), Ok(()));
        assert_eq!(validate_slug("/", &reserved), Ok(()));
        assert_eq!(validate_slug("///", &reserved), Ok(()));

        // Subpath slugs
        assert_eq!(validate_slug("20/10", &reserved), Ok(()));
        assert_eq!(validate_slug("/20/10/", &reserved), Ok(()));
        assert_eq!(validate_slug("shop/ao/so-mi", &reserved), Ok(()));

        // Single segments
        assert_eq!(validate_slug("demo", &reserved), Ok(()));
        assert_eq!(validate_slug("shop-123", &reserved), Ok(()));
        assert_eq!(validate_slug("my_site", &reserved), Ok(()));
        assert_eq!(validate_slug("ab", &reserved), Ok(()));
        assert_eq!(validate_slug("a", &reserved), Ok(()));
    }

    #[test]
    fn test_invalid_slugs() {
        let reserved = vec!["api".to_string(), "admin".to_string()];
        assert_eq!(validate_slug("-demo", &reserved), Err(SlugValidationError::InvalidCharacters));
        assert_eq!(validate_slug("demo-", &reserved), Err(SlugValidationError::InvalidCharacters));
        assert_eq!(validate_slug("demo site", &reserved), Err(SlugValidationError::InvalidCharacters));
        assert_eq!(validate_slug("20/../10", &reserved), Err(SlugValidationError::InvalidCharacters));
        assert_eq!(validate_slug("demo.com", &reserved), Err(SlugValidationError::InvalidCharacters));
        assert_eq!(validate_slug("api", &reserved), Err(SlugValidationError::Reserved("api".into())));
        assert_eq!(validate_slug("admin/dashboard", &reserved), Err(SlugValidationError::Reserved("admin".into())));
    }

    #[test]
    fn test_normalize_slug() {
        assert_eq!(normalize_slug(""), "");
        assert_eq!(normalize_slug("/"), "");
        assert_eq!(normalize_slug("///"), "");
        assert_eq!(normalize_slug("  /20/10/  "), "20/10");
        assert_eq!(normalize_slug("SHOP//AO"), "shop/ao");
    }

    #[test]
    fn test_sanitize_slug() {
        assert_eq!(sanitize_suggested_slug("My Cool Website!"), "my-cool-website");
        assert_eq!(sanitize_suggested_slug("---demo---"), "demo");
        assert_eq!(sanitize_suggested_slug("20/10 Festival!"), "20/10-festival");
    }
}
