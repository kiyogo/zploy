use zploy::security::{normalize_slug, sanitize_suggested_slug, validate_slug, SlugValidationError};

#[test]
fn test_slug_valid_formats() {
    let reserved = vec!["api".to_string(), "admin".to_string(), "login".to_string()];

    // Root slug (empty or "/")
    assert!(validate_slug("", &reserved).is_ok());
    assert!(validate_slug("   ", &reserved).is_ok());
    assert!(validate_slug("/", &reserved).is_ok());
    assert!(validate_slug("///", &reserved).is_ok());

    // Subpath / nested slugs with '/'
    assert!(validate_slug("20/10", &reserved).is_ok());
    assert!(validate_slug("/20/10/", &reserved).is_ok());
    assert!(validate_slug("demo/test", &reserved).is_ok());
    assert!(validate_slug("shop/ao/so-mi", &reserved).is_ok());

    // Single segments
    assert!(validate_slug("demo", &reserved).is_ok());
    assert!(validate_slug("my-shop", &reserved).is_ok());
    assert!(validate_slug("game_2026", &reserved).is_ok());
    assert!(validate_slug("ab", &reserved).is_ok());
    assert!(validate_slug("a", &reserved).is_ok());
    assert!(validate_slug("v1-release-test", &reserved).is_ok());
}

#[test]
fn test_slug_invalid_formats() {
    let reserved = vec!["api".to_string(), "admin".to_string(), "login".to_string()];

    // Starting/ending with hyphens or underscores
    assert_eq!(validate_slug("-demo", &reserved), Err(SlugValidationError::InvalidCharacters));
    assert_eq!(validate_slug("demo-", &reserved), Err(SlugValidationError::InvalidCharacters));
    assert_eq!(validate_slug("_demo", &reserved), Err(SlugValidationError::InvalidCharacters));
    assert_eq!(validate_slug("20/-10", &reserved), Err(SlugValidationError::InvalidCharacters));

    // Illegal characters
    assert_eq!(validate_slug("demo.com", &reserved), Err(SlugValidationError::InvalidCharacters));
    assert_eq!(validate_slug("demo test", &reserved), Err(SlugValidationError::InvalidCharacters));
    assert_eq!(validate_slug("demo$#", &reserved), Err(SlugValidationError::InvalidCharacters));
    assert_eq!(validate_slug("20/../10", &reserved), Err(SlugValidationError::InvalidCharacters));

    // Reserved words
    assert_eq!(validate_slug("api", &reserved), Err(SlugValidationError::Reserved("api".into())));
    assert_eq!(validate_slug("API", &reserved), Err(SlugValidationError::Reserved("api".into())));
    assert_eq!(validate_slug("admin", &reserved), Err(SlugValidationError::Reserved("admin".into())));
    assert_eq!(validate_slug("admin/subpath", &reserved), Err(SlugValidationError::Reserved("admin".into())));
}

#[test]
fn test_slug_normalization() {
    assert_eq!(normalize_slug(""), "");
    assert_eq!(normalize_slug("/"), "");
    assert_eq!(normalize_slug("///"), "");
    assert_eq!(normalize_slug("  /20/10/  "), "20/10");
    assert_eq!(normalize_slug("DEMO//TEST"), "demo/test");
}

#[test]
fn test_slug_sanitizer() {
    assert_eq!(sanitize_suggested_slug("My Cool Portfolio!"), "my-cool-portfolio");
    assert_eq!(sanitize_suggested_slug("---test_site---"), "test_site");
    assert_eq!(sanitize_suggested_slug("20/10 Festival!"), "20/10-festival");
}
