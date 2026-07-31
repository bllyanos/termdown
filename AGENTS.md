# termdown Project Instructions

## Project shape

- Rust 2024 terminal application.
- CLI parsing lives in `src/main.rs`.
- Markdown rendering lives in `src/markdown.rs`.
- Application state and input handling live in `src/app.rs`.
- Ratatui presentation lives in `src/ui.rs`.

## Development checks

Run these before submitting changes:

```bash
cargo fmt -- --check
cargo test
```

For manual CLI checks:

```bash
cargo run -- --help
cargo run -- --version
```

The normal invocation requires a Markdown `FILE`; `-` reads standard input.

## Commit conventions

Use Conventional Commits:

- `feat:` for user-visible features; bumps the minor version before 1.0.
- `fix:` for user-visible bug fixes; bumps the patch version.
- `BREAKING CHANGE:` or `!` for breaking changes; bumps the major version.
- `docs:`, `ci:`, `chore:`, `refactor:`, and `test:` do not normally create a release.

Keep commit subjects imperative and concise.

## Releases

`.github/workflows/release.yml` uses Release Please to read Conventional Commits on `main`. It opens a release PR, updates Cargo metadata and the changelog, and creates the version tag and GitHub release when the PR is merged. The same workflow builds and uploads the Linux x86_64 archive and checksum.

Do not manually edit the package version for routine releases. Verify release PRs contain the expected `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md` updates before merging.
