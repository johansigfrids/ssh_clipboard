# Repository Guidelines

## Project Structure & Module Organization

- `src/` holds the Rust source code. The current entry point is `src/main.rs`.
- `Cargo.toml` defines the crate metadata and dependencies; `Cargo.lock` pins versions.
- `target/` is generated build output and should not be edited by hand.

As the project grows, prefer adding Rust modules under `src/` (for example `src/client.rs`, `src/server.rs`) and wire them from `main.rs` or a new `lib.rs`.

## Build, Test, and Development Commands

- `cargo build` — compiles the project in debug mode.
- `cargo run` — builds and runs the binary.
- `cargo test` — runs the test suite (currently no tests are defined).
- `cargo clippy --features agent -- -D warnings` — lint the codebase (CI-required).
- `cargo build --release` — produces optimized release binaries in `target/release/`.

## Coding Style & Naming Conventions

- Use standard Rust formatting (4-space indentation, rustfmt defaults). If you format, use `cargo fmt`.
- Prefer `snake_case` for functions and variables, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Keep modules small and focused; name files after their primary responsibility (e.g., `clipboard.rs`, `ssh.rs`).

## Testing Guidelines

- No test framework is configured beyond Rust’s built-in test harness.
- For new tests, place unit tests in `mod tests` within the relevant module, or integration tests in `tests/`.
- Name test functions descriptively (e.g., `copies_clipboard_over_ssh`).

## Commit & Pull Request Guidelines

- Current history uses short, imperative summaries (e.g., “Add readme”, “init”). Keep commit messages concise and action-oriented.
- Pull requests should include: a clear description of changes, how to run/verify them, and any relevant usage notes.
- If behavior changes, include updated docs or examples in `README.md`.

## Documentation Knowledge Base

- Docs entrypoint: `docs/index.md` (follow `docs/writing-docs.md`).
- At minimum, read `docs/index.md` before making changes; for major changes, skim relevant `docs/*.md` for context.
- As you learn: update existing docs or add `docs/<topic>.md`, then link it in `docs/index.md`.
- Keep `ARCHITECTURE.md` up to date as the system design or component boundaries change.
- After behavior changes or bug fixes: refresh related docs and keep cross-links current.
- Do not include secrets in docs; reference where secrets/config are managed instead.

## Security & Configuration Tips

- This project communicates over SSH; avoid committing secrets or host-specific configs.
- Prefer environment variables or local config files (excluded via `.gitignore`) for machine-specific settings.
