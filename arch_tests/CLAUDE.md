# arch_tests — dependency direction enforcement

`src/lib.rs` is an empty shell; all assertions live in `tests/architecture.rs`, which parses each crate's `Cargo.toml` and `lib.rs` directly.

## The two enforced rule classes

**Dependency direction** (reads `[dependencies]` of `Cargo.toml`; for `usecases`, tokio is banned only in the runtime table — the dev table is allowed)

- `frameworks` ✗ `usecases` / `entities`
- `usecases` ✗ `adapters` / `frameworks` / `axum` / `serde` / `serde_json` / runtime `tokio`
- `entities` ✗ `usecases` / `adapters` / `frameworks` / `serde` / `serde_json` / `tokio`
- `adapters` ✗ `frameworks`

**Re-exports must be named** (reads `lib.rs`; bans `pub use inner::*;`)

- `usecases` re-exports `entities` by name; `adapters` re-exports `usecases` by name
- `frameworks` can reach inner-layer types only through `adapters`' named re-exports — when adding a new type, add a line in `adapters/src/lib.rs`; don't take the shortcut of a glob

## Layer rules

- For new bans use the existing `assert_no_dependency` / `assert_no_runtime_dependency` / `assert_no_wildcard_reexport`, one assertion per `#[test]`
- These helpers have their own unit tests (`dependency_names_*` / `wildcard_reexport_violation_*`) — required by the coverage gate; add tests in sync when changing a helper
