# arch_tests — 依赖方向强制

`src/lib.rs` 是空壳，全部断言在 `tests/architecture.rs`，直接解析各 crate 的 `Cargo.toml` 与 `lib.rs`。

## 强制的两类规则

**依赖方向**（读 `Cargo.toml` 的 `[dependencies]`，`usecases` 的 tokio 只禁 runtime 表、dev 表放行）

- `frameworks` ✗ `usecases` / `entities`
- `usecases` ✗ `adapters` / `frameworks` / `axum` / `serde` / `serde_json` / runtime `tokio`
- `entities` ✗ `usecases` / `adapters` / `frameworks` / `serde` / `serde_json` / `tokio`
- `adapters` ✗ `frameworks`

**再导出必须具名**（读 `lib.rs`，禁 `pub use inner::*;`）

- `usecases` 具名再导出 `entities`，`adapters` 具名再导出 `usecases`
- `frameworks` 拿内层类型只能走 `adapters` 的具名再导出——加新类型时要在 `adapters/src/lib.rs` 补一行，别图省事写 glob

## 本层规则

- 新增禁令用现成的 `assert_no_dependency` / `assert_no_runtime_dependency` / `assert_no_wildcard_reexport`，一条断言一个 `#[test]`
- 这些 helper 自身也有单测（`dependency_names_*` / `wildcard_reexport_violation_*`）——覆盖率门禁要求，改 helper 时同步补
