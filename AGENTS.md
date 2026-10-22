# Repository Guidelines

## Project Structure & Module Organization

- `src/` contains the headless shared library, session CLI and shelf/server services. Entry points are `src/main.rs`, `src/lib.rs`.
- `tests/` contains CLI and process integration tests; Rust modules also contain unit/property tests.
- `desktop/` contains Svelte/TypeScript, Node release tooling and a separate Tauri Rust project with its own lockfile.
- `Cargo.toml` builds a single headless core/CLI; native clipboard/tray/startup code belongs to `desktop/`.
- Cargo `target/`, frontend build output and ignored `desktop/artifacts/` are generated output; do not edit them by hand.

## Build, Test, and Development Commands

- `cargo build` — compiles the project in debug mode.
- `cargo run --bin ssh_clipboard -- --help` — inspect the CLI command surface.
- `cargo test --locked` — run core/session CLI tests.
- Linux socket/service integration tests require Linux or the established WSL toolchain.
- `cargo fmt` — format the codebase with rustfmt.
- `cargo clippy --locked --all-targets -- -D warnings` — lint the codebase (CI-required).
- `cargo build --release` — produces optimized release binaries in `target/release/`.
- From `desktop/`, `npm test` runs Node tooling and Vitest/Svelte interactions; `npm run build` runs Svelte checks and Vite.
- [Testing](docs/testing.md) has desktop Rust, platform/package and documentation-only checks. Run checks appropriate to the changed component; builds and rendered UI tests do not prove native clipboard, login or installer behavior.

## Coding Style & Naming Conventions

- Use standard Rust formatting (4-space indentation, rustfmt defaults). If you format, use `cargo fmt`.
- Prefer `snake_case` for functions and variables, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Keep modules small and focused; name files after their primary responsibility (e.g., `clipboard.rs`, `ssh.rs`).
- Keep platform `#[cfg(...)]` usage consistent: if a helper is only used in a platform-gated branch, gate the helper similarly (or with `#[cfg(any(..., test))]` when tests need it).

## Testing Guidelines

- Rust uses its built-in test harness and `proptest`; desktop frontend/tooling uses Vitest, Testing Library and Node's test runner.
- For new tests, place unit tests in `mod tests` within the relevant module, or integration tests in `tests/`.
- Name test functions descriptively (e.g., `copies_clipboard_over_ssh`).

## Commit & Pull Request Guidelines

- Keep commit messages concise and action-oriented.
- Pull requests should include: a clear description of changes, how to run/verify them, and any relevant usage notes.
- If behavior changes, include updated docs or examples in [README.md](README.md).

## Documentation Knowledge Base

- Docs entrypoint: [docs/index.md](docs/index.md); follow [docs/writing-docs.md](docs/writing-docs.md).
- At minimum, read [docs/index.md](docs/index.md) before making changes; for major changes, skim relevant `docs/*.md` for context.
- Update the current feature/runbook that owns a contract, rather than copying release chronology into several guides. Add new documents to the index.
- `docs/history/` preserves dated plans, acceptance and exact source/package evidence. Archived 0.4 compatibility workflows live under `docs/history/0.4/`. Historical omissions are not automatic new manual-test assignments.
- Keep [ARCHITECTURE.md](ARCHITECTURE.md) up to date as the system design or component boundaries change.
- After behavior changes or bug fixes: refresh related docs and keep cross-links current.
- When fixing issues, capture the prevention guidance in docs (root cause + guardrail) so the same mistake is less likely to recur.
- Do not include secrets in docs; reference where secrets/config are managed instead.

## Security & Configuration Tips

- This project communicates over SSH; avoid committing secrets or host-specific configs.
- Prefer environment variables or local config files (excluded via `.gitignore`) for machine-specific settings.
