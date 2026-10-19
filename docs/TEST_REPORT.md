# Zploy Automated Test Suite Report

All automated tests in the test suite execute against a live test environment.

## Test Summary

* Total Test Suites: 4
* Total Unit & Integration Tests: 21
* Passed: 21 (100%)
* Failed: 0
* Execution Mode: `cargo test -- --test-threads=1`

## Test Details

### 1. Slug Validation (`tests/slug_test.rs`)
* `test_slug_valid_formats` (Root slug, subpaths like `shop`, `20/10`) &mdash; Passed
* `test_slug_invalid_formats` (Special characters, reserved paths `admin`, `api`) &mdash; Passed
* `test_slug_normalization` (Trim slashes, lowercase conversion) &mdash; Passed
* `test_slug_sanitizer` (Filename to valid slug suggestion) &mdash; Passed

### 2. Security & Archive Processing (`tests/security_validation_test.rs`)
* `test_zip_slip_attack_rejected` (Relative escape paths `../../`) &mdash; Passed
* `test_zip_single_root_folder_flattening` (Single nested directory unpack) &mdash; Passed
* `test_zip_missing_index_html` (Enforcing `index.html` presence) &mdash; Passed
* `test_disallowed_extension_rejection` (Rejecting `.php`, `.exe`, `.py`) &mdash; Passed
* `test_path_traversal_attacks` (Jailbreak prevention in URL routes) &mdash; Passed

### 3. End-to-End System Integration (`tests/integration_test.rs`)
* `test_health_check` (Service status and version) &mdash; Passed
* `test_site_lifecycle_and_static_serving` (Create site -> Deploy v1 -> Verify assets -> Deploy v2 -> Rollback -> Delete) &mdash; Passed
* `test_root_site_and_nested_slug_serving` (Root `/` and nested `/20/10/`) &mdash; Passed
* `test_slug_availability_api` (API path collision detection) &mdash; Passed

### 4. Core Security Unit Tests (`src/lib.rs`)
* `test_password_hashing_and_verification` (Argon2id) &mdash; Passed
* `test_token_generation_and_hashing` (CSPRNG + SHA-256) &mdash; Passed
* `test_path_security_checks` &mdash; Passed
* `test_detect_common_root` &mdash; Passed
