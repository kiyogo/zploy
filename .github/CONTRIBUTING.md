# Contributing to Zploy

Thank you for your interest in contributing to Zploy! We welcome contributions of all kinds: bug reports, feature requests, documentation improvements, code changes, and security disclosures.

## Getting Started

1. Fork the repository on GitHub: `https://github.com/kiyogo/zploy`
2. Clone your fork locally:
   ```bash
   git clone https://github.com/your-username/zploy.git
   cd zploy
   ```
3. Set up a local MySQL/MariaDB database:
   ```sql
   CREATE DATABASE zploy CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
   ```
4. Copy `config/config.example.toml` to `config/config.toml` and configure your database credentials.
5. Build and run tests:
   ```bash
   cargo test
   ```

## Development Workflow

1. Create a feature branch:
   ```bash
   git checkout -b feature/my-new-feature
   ```
2. Make your changes with clean, well-tested Rust code.
3. Ensure formatting and clippy pass:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   ```
4. Run all unit and integration tests:
   ```bash
   cargo test
   ```
5. Commit your changes with conventional commit messages:
   ```bash
   git commit -m "feat: add support for custom domain routing"
   ```
6. Push to your branch and open a Pull Request.

## Code Standards

* Idiomatic Rust: Adhere to standard Rust idioms and the Tokio/Axum ecosystem conventions.
* Security First: All file operations must respect path traversal protections, quota bounds, and extension whitelists.
* Zero Downtime: Database schema updates should use atomic migrations.
* Clear Documentation: Update README.md and API documentation for any user-facing changes.

## License

By contributing to Zploy, you agree that your contributions will be licensed under the project's MIT License.
