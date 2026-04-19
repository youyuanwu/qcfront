# Copilot Instructions

## Git

- **Never** auto `git add` or `git commit`. Always wait for the user to explicitly ask.
- Do not stage, commit, or push changes unless the user says so.

## Rust

- This is a Cargo workspace. All crates live under `crates/`.
- Use `cargo build`, `cargo test`, `cargo run -p <crate> --bin <name>` for builds.
- Prefer existing crates from crates.io over reimplementing standard algorithms.

## Documentation

- Background research docs go in `docs/background/`.
- Feature design docs go in `docs/features/`.
- Keep docs factual and cite sources.
